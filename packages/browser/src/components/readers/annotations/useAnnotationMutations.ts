import { useGraphQLMutation } from '@stump/client'
import { AnnotationStyle, graphql, type ReadiumLocatorInput } from '@stump/graphql'
import { type QueryKey, useQueryClient } from '@tanstack/react-query'
import { useCallback, useMemo, useSyncExternalStore } from 'react'
import { toast } from 'sonner'

import { useTranslate } from '@/hooks/useTranslate'

import type { ReaderAnnotation, ReaderLocator } from './types'

const _createMutation = graphql(`
	mutation CreateReaderAnnotation($input: CreateAnnotationInput!) {
		createAnnotation(input: $input) {
			id
			mediaId
			userId
			annotationText
			style
			createdAt
			updatedAt
			locator {
				chapterTitle
				href
				title
				type
				locations {
					fragments
					progression
					position
					totalProgression
					cssSelector
					rects {
						x
						y
						width
						height
					}
				}
				text {
					after
					before
					highlight
				}
			}
		}
	}
`)

const _updateMutation = graphql(`
	mutation UpdateReaderAnnotation($input: UpdateAnnotationInput!) {
		updateAnnotation(input: $input) {
			id
			annotationText
			style
			updatedAt
		}
	}
`)

const _deleteMutation = graphql(`
	mutation DeleteReaderAnnotation($id: String!) {
		deleteAnnotation(id: $id) {
			id
		}
	}
`)

/**
 * Where a reader keeps its annotations in the query cache: the query that loaded them and
 * how to read and replace the list inside that query's data. The EPUB reader keeps them in
 * its `epubById` query; another reader can point this at its own query.
 */
export type AnnotationCacheAdapter<TData> = {
	queryKey: QueryKey
	read: (data: TData | undefined) => GraphQLAnnotationLike[] | null | undefined
	write: (data: TData, annotations: ReaderAnnotation[]) => TData
}

type UseAnnotationMutationsArgs<TData> = {
	mediaId: string
	isIncognito: boolean
	/** Used until the cache holds the reader's query (and in tests) */
	initialAnnotations: ReaderAnnotation[]
	cache: AnnotationCacheAdapter<TData>
}

export type AnnotationChanges = {
	/** `null` removes the note, `undefined` keeps it */
	annotationText?: string | null
	style?: AnnotationStyle
}

/**
 * GraphQL CRUD for a reader's annotations. Changes are applied to the query cache first
 * (so decorations and lists update immediately) and rolled back if the server refuses.
 * Annotations belong to the signed-in user; incognito reading doesn't save any.
 */
export function useAnnotationMutations<TData>({
	mediaId,
	isIncognito,
	initialAnnotations,
	cache,
}: UseAnnotationMutationsArgs<TData>) {
	const { translate } = useTranslate()
	const queryClient = useQueryClient()
	const { queryKey, read, write } = cache

	const cachedAnnotations = useSyncExternalStore(
		(onStoreChange) => queryClient.getQueryCache().subscribe(onStoreChange),
		() => read(queryClient.getQueryData<TData>(queryKey)) ?? null,
		() => null,
	)
	const annotations = useMemo(
		() => cachedAnnotations?.map(graphqlAnnotationToReaderAnnotation) ?? initialAnnotations,
		[cachedAnnotations, initialAnnotations],
	)

	const updateCachedAnnotations = useCallback(
		(transform: (annotations: ReaderAnnotation[]) => ReaderAnnotation[]) => {
			queryClient.setQueryData<TData>(queryKey, (data) => {
				const current = read(data)
				if (!data || !current) return data
				return write(data, transform(current.map(graphqlAnnotationToReaderAnnotation)))
			})
		},
		[queryClient, queryKey, read, write],
	)

	const snapshot = useCallback(async () => {
		await queryClient.cancelQueries({ queryKey })
		return queryClient.getQueryData<TData>(queryKey)
	}, [queryClient, queryKey])

	const { mutate: createMutation, isPending: isCreating } = useGraphQLMutation(_createMutation, {
		mutationKey: ['createReaderAnnotation', mediaId],
		onMutate: async ({ input }) => {
			const previous = await snapshot()
			const optimisticId = makeOptimisticId()
			const now = new Date().toISOString()

			updateCachedAnnotations((annotations) => [
				...annotations,
				{
					id: optimisticId,
					mediaId,
					userId: '',
					annotationText: input.annotationText ?? null,
					style: input.style ?? AnnotationStyle.Highlight,
					createdAt: now,
					updatedAt: now,
					locator: locatorInputToReaderLocator(input.locator),
				},
			])

			return { optimisticId, previous }
		},
		onSuccess: ({ createAnnotation }, _variables, context) => {
			updateCachedAnnotations((annotations) =>
				annotations.map((annotation) =>
					annotation.id === context.optimisticId
						? graphqlAnnotationToReaderAnnotation(createAnnotation)
						: annotation,
				),
			)
		},
		onError: (error, _variables, context) => {
			queryClient.setQueryData(queryKey, context?.previous)
			console.error('[useAnnotationMutations] createAnnotation failed', error)
			toast.error(translate('shared.epubReader.annotation.saveFailed'))
		},
		onSettled: () => queryClient.invalidateQueries({ queryKey }),
	})

	const { mutate: updateMutation, isPending: isUpdating } = useGraphQLMutation(_updateMutation, {
		mutationKey: ['updateReaderAnnotation', mediaId],
		onMutate: async ({ input }) => {
			const previous = await snapshot()

			updateCachedAnnotations((annotations) =>
				annotations.map((annotation) =>
					annotation.id === input.id
						? {
								...annotation,
								annotationText:
									input.annotationText === undefined
										? annotation.annotationText
										: input.annotationText,
								style: input.style ?? annotation.style,
								updatedAt: new Date().toISOString(),
							}
						: annotation,
				),
			)

			return { previous }
		},
		onError: (error, _variables, context) => {
			queryClient.setQueryData(queryKey, context?.previous)
			console.error('[useAnnotationMutations] updateAnnotation failed', error)
			toast.error(translate('shared.epubReader.annotation.updateFailed'))
		},
		onSettled: () => queryClient.invalidateQueries({ queryKey }),
	})

	const { mutate: deleteMutation, isPending: isDeleting } = useGraphQLMutation(_deleteMutation, {
		mutationKey: ['deleteReaderAnnotation', mediaId],
		onMutate: async ({ id }) => {
			const previous = await snapshot()
			updateCachedAnnotations((annotations) =>
				annotations.filter((annotation) => annotation.id !== id),
			)
			return { previous }
		},
		onError: (error, _variables, context) => {
			queryClient.setQueryData(queryKey, context?.previous)
			console.error('[useAnnotationMutations] deleteAnnotation failed', error)
			toast.error(translate('shared.epubReader.annotation.deleteFailed'))
		},
		onSettled: () => queryClient.invalidateQueries({ queryKey }),
	})

	const createAnnotation = useCallback(
		(
			locator: ReaderLocator,
			{ annotationText, style = AnnotationStyle.Highlight }: AnnotationChanges = {},
		) => {
			if (isIncognito) {
				toast.info(translate('shared.epubReader.annotation.disabledInIncognito'))
				return undefined
			}

			return createMutation({
				input: {
					mediaId,
					locator: readerLocatorToInput(locator),
					annotationText: annotationText || undefined,
					style,
				},
			})
		},
		[mediaId, isIncognito, createMutation, translate],
	)

	const updateAnnotation = useCallback(
		(id: string, { annotationText, style }: AnnotationChanges) => {
			if (isIncognito) return

			return updateMutation({
				input: {
					id,
					// Omitted keys are left unchanged by the server
					...(annotationText !== undefined ? { annotationText: annotationText || null } : {}),
					...(style ? { style } : {}),
				},
			})
		},
		[isIncognito, updateMutation],
	)

	const deleteAnnotation = useCallback(
		(id: string) => {
			if (isIncognito) return

			return deleteMutation({ id })
		},
		[isIncognito, deleteMutation],
	)

	return {
		annotations,
		createAnnotation,
		updateAnnotation,
		deleteAnnotation,
		isPending: isCreating || isUpdating || isDeleting,
	}
}

function makeOptimisticId(): string {
	return `optimistic-${Date.now()}-${Math.random().toString(36).slice(2)}`
}

/** Whether an annotation is still being saved (its ID is a placeholder) */
export function isOptimisticAnnotationId(id: string): boolean {
	return id.startsWith('optimistic-')
}

export function readerLocatorToInput(locator: ReaderLocator): ReadiumLocatorInput {
	const locations = locator.locations
	return {
		chapterTitle: locator.chapterTitle ?? locator.title ?? '',
		href: locator.href,
		title: locator.title ?? undefined,
		type: locator.type || 'application/xhtml+xml',
		locations: locations
			? {
					fragments: locations.fragments ?? undefined,
					position: locations.position ?? undefined,
					progression: locations.progression ?? undefined,
					totalProgression: locations.totalProgression ?? undefined,
					cssSelector: locations.cssSelector ?? undefined,
					rects: locations.rects?.map(({ x, y, width, height }) => ({ x, y, width, height })),
				}
			: undefined,
		text: locator.text
			? {
					after: locator.text.after ?? undefined,
					before: locator.text.before ?? undefined,
					highlight: locator.text.highlight ?? undefined,
				}
			: undefined,
	}
}

function locatorInputToReaderLocator(locator: ReadiumLocatorInput): ReaderLocator {
	return graphQLLocatorToReaderLocator(locator as GraphQLLocatorLike)
}

type MaybeDecimal = number | string | null | undefined

type GraphQLLocatorLike = {
	href: string
	type?: string | null
	title?: string | null
	chapterTitle?: string | null
	locations?: {
		fragments?: string[] | null
		progression?: MaybeDecimal
		position?: number | null
		totalProgression?: MaybeDecimal
		cssSelector?: string | null
		rects?: Array<{
			x: MaybeDecimal
			y: MaybeDecimal
			width: MaybeDecimal
			height: MaybeDecimal
		}> | null
	} | null
	text?: {
		after?: string | null
		before?: string | null
		highlight?: string | null
	} | null
}

/** An annotation as GraphQL returns it (decimals may be strings, style may be missing) */
export type GraphQLAnnotationLike = {
	id: string
	mediaId: string
	userId: string
	annotationText?: string | null
	style?: AnnotationStyle | null
	createdAt: string
	updatedAt: string
	locator: GraphQLLocatorLike
}

/**
 * Maps a `MediaAnnotation` / `MediaAnnotationModel` GraphQL shape (mutation results or a
 * reader query's `annotations` field) onto a {@link ReaderAnnotation}
 */
export function graphqlAnnotationToReaderAnnotation(
	annotation: GraphQLAnnotationLike,
): ReaderAnnotation {
	return {
		id: annotation.id,
		mediaId: annotation.mediaId,
		userId: annotation.userId,
		annotationText: annotation.annotationText ?? null,
		style: annotation.style ?? AnnotationStyle.Highlight,
		createdAt: annotation.createdAt,
		updatedAt: annotation.updatedAt,
		locator: graphQLLocatorToReaderLocator(annotation.locator),
	}
}

export function graphQLLocatorToReaderLocator(locator: GraphQLLocatorLike): ReaderLocator {
	const locations = locator.locations
	return {
		href: locator.href,
		type: locator.type || 'application/xhtml+xml',
		title: locator.title ?? undefined,
		chapterTitle: locator.chapterTitle ?? undefined,
		locations: locations
			? {
					fragments: locations.fragments ?? undefined,
					progression: toNumber(locations.progression),
					position: locations.position ?? undefined,
					totalProgression: toNumber(locations.totalProgression),
					cssSelector: locations.cssSelector ?? undefined,
					rects: locations.rects
						? locations.rects.map((rect) => ({
								x: toNumber(rect.x) ?? 0,
								y: toNumber(rect.y) ?? 0,
								width: toNumber(rect.width) ?? 0,
								height: toNumber(rect.height) ?? 0,
							}))
						: undefined,
				}
			: undefined,
		text: locator.text
			? {
					after: locator.text.after ?? undefined,
					before: locator.text.before ?? undefined,
					highlight: locator.text.highlight ?? undefined,
				}
			: undefined,
	}
}

function toNumber(value: MaybeDecimal): number | undefined {
	if (value == null || value === '') return undefined
	const num = Number(value)
	return Number.isFinite(num) ? num : undefined
}
