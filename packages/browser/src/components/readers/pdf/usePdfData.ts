import { useGraphQL, useGraphQLMutation, useSDK } from '@stump/client'
import { graphql, PdfReaderAnnotationsQuery } from '@stump/graphql'
import type { PdfLayout, PdfPageText } from '@stump/sdk'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { useCallback, useMemo } from 'react'

import {
	type AnnotationCacheAdapter,
	graphQLLocatorToReaderLocator,
	type ReaderAnnotation,
	type ReaderLocator,
	readerLocatorToInput,
	useAnnotationMutations,
} from '../annotations'

/** The document layout: page sizes, outline, labels, text availability */
export function usePdfLayout(id: string) {
	const { sdk } = useSDK()
	return useQuery<PdfLayout>({
		queryKey: ['pdfLayout', id],
		queryFn: ({ signal }) => sdk.pdf.layout(id, signal),
		staleTime: 60_000,
		retry: 1,
	})
}

export function pdfTextQueryKey(id: string, v: string | undefined, page: number) {
	return ['pdfText', id, v ?? '', page] as const
}

/** A page's text layer (1-based page). Cached for good: the URL carries the document version */
export function usePdfPageText(id: string, v: string | undefined, page: number, enabled: boolean) {
	const { sdk } = useSDK()
	return useQuery<PdfPageText>({
		queryKey: pdfTextQueryKey(id, v, page),
		queryFn: ({ signal }) => sdk.pdf.text(id, page, v, signal),
		enabled,
		staleTime: Infinity,
		gcTime: 5 * 60_000,
		retry: 1,
	})
}

/** Fetches a page's text layer ahead of time (ignored if it fails) */
export function usePrefetchPdfText(id: string, v: string | undefined) {
	const { sdk } = useSDK()
	const client = useQueryClient()
	return useCallback(
		(page: number) =>
			client
				.prefetchQuery({
					queryKey: pdfTextQueryKey(id, v, page),
					queryFn: ({ signal }) => sdk.pdf.text(id, page, v, signal, { prefetch: true }),
					staleTime: Infinity,
					retry: false,
				})
				.catch(() => undefined),
		[client, id, sdk, v],
	)
}

const annotationsQuery = graphql(`
	query PdfReaderAnnotations($id: ID!) {
		annotationsByMediaId(id: $id) {
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

const NO_ANNOTATIONS: ReaderAnnotation[] = []

/** The signed-in user's annotations on a PDF, with optimistic create/update/delete */
export function usePdfAnnotations(mediaId: string, isIncognito: boolean) {
	const queryKey = useMemo(() => ['pdfAnnotations', mediaId], [mediaId])
	useGraphQL(annotationsQuery, queryKey, { id: mediaId }, { staleTime: 30_000 })

	const cache = useMemo<AnnotationCacheAdapter<PdfReaderAnnotationsQuery>>(
		() => ({
			queryKey,
			read: (data) => data?.annotationsByMediaId,
			write: (data, annotations) => ({
				...data,
				annotationsByMediaId:
					annotations as unknown as PdfReaderAnnotationsQuery['annotationsByMediaId'],
			}),
		}),
		[queryKey],
	)

	return useAnnotationMutations({
		mediaId,
		isIncognito,
		initialAnnotations: NO_ANNOTATIONS,
		cache,
	})
}

const bookmarksQuery = graphql(`
	query PdfReaderBookmarks($id: ID!) {
		bookmarksByMediaId(id: $id) {
			id
			page
			previewContent
			createdAt
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
				}
			}
		}
	}
`)

const createBookmarkMutation = graphql(`
	mutation CreatePdfBookmark($input: BookmarkInput!) {
		createBookmark(input: $input) {
			id
		}
	}
`)

const deleteBookmarkMutation = graphql(`
	mutation DeletePdfBookmark($id: String!) {
		deleteBookmark(id: $id) {
			id
		}
	}
`)

export type PdfBookmark = {
	id: string
	/** 1-based page */
	page: number
	previewContent?: string | null
	createdAt: string
	locator: ReaderLocator | null
}

/** The signed-in user's bookmarks on a PDF (one per page) */
export function usePdfBookmarks(mediaId: string) {
	const client = useQueryClient()
	const queryKey = useMemo(() => ['pdfBookmarks', mediaId], [mediaId])
	const { data } = useGraphQL(bookmarksQuery, queryKey, { id: mediaId }, { staleTime: 30_000 })

	const bookmarks = useMemo<PdfBookmark[]>(
		() =>
			(data?.bookmarksByMediaId ?? [])
				.map((bookmark) => {
					const locator = bookmark.locator ? graphQLLocatorToReaderLocator(bookmark.locator) : null
					const page =
						locator?.locations?.position ?? (bookmark.page && bookmark.page > 0 ? bookmark.page : 0)
					return {
						id: bookmark.id,
						page,
						previewContent: bookmark.previewContent,
						createdAt: bookmark.createdAt,
						locator,
					}
				})
				.filter((bookmark) => bookmark.page > 0)
				.sort((a, b) => a.page - b.page),
		[data],
	)

	const invalidate = useCallback(() => client.invalidateQueries({ queryKey }), [client, queryKey])
	const { mutate: create, isPending: isCreating } = useGraphQLMutation(createBookmarkMutation, {
		onSettled: invalidate,
	})
	const { mutate: remove, isPending: isDeleting } = useGraphQLMutation(deleteBookmarkMutation, {
		onSettled: invalidate,
	})

	const createBookmark = useCallback(
		(locator: ReaderLocator, previewContent?: string) =>
			create({
				input: {
					mediaId,
					locator: readerLocatorToInput(locator),
					previewContent: previewContent || undefined,
				},
			}),
		[create, mediaId],
	)
	const deleteBookmark = useCallback((id: string) => remove({ id }), [remove])

	return { bookmarks, createBookmark, deleteBookmark, isPending: isCreating || isDeleting }
}
