import { useGraphQL, useGraphQLMutation, useSDK } from '@stump/client'
import {
	extractErrorMessage,
	IntakeReviewInput,
	IntakeState,
	IntakeUploadMode,
	useFragment,
} from '@stump/graphql'
import { intakeUploadErrorOf } from '@stump/sdk'
import { keepPreviousData, useQueryClient } from '@tanstack/react-query'
import { useCallback, useEffect, useRef, useState } from 'react'
import { toast } from 'sonner'

import {
	intakeAcceptMutation,
	intakeApproveMutation,
	intakeChangeLibraryMutation,
	intakeDiscardMutation,
	type IntakeItemData,
	intakeItemFragment,
	intakeItemsQuery,
	type IntakeLimitsData,
	intakeReanalyseMutation,
	intakeRejectMutation,
	intakeReviewMutation,
	intakeSubmitMutation,
	intakeUploadLimitsQuery,
	intakeUploaderAgeQuery,
	intakeWithdrawMutation,
	pendingIntakeCountQuery,
	pendingIntakeItemsQuery,
} from './queries'
import {
	type AgeLimit,
	genresDiffer,
	type PendingFile,
	planUploads,
	pollInterval,
	uploadPercent,
	type UploadBatch,
	uploadProblem,
} from './reviewState'

export const INTAKE_ITEMS_KEY = ['intakeItems'] as const
export const INTAKE_LIMITS_KEY = ['intakeUploadLimits'] as const
export const PENDING_INTAKE_KEY = ['pendingIntakeItems'] as const
export const PENDING_INTAKE_COUNT_KEY = ['pendingIntakeCount'] as const

/** Every query that shows something of the intake: a change in one can change the others */
const INTAKE_KEYS = [
	INTAKE_ITEMS_KEY,
	INTAKE_LIMITS_KEY,
	PENDING_INTAKE_KEY,
	PENDING_INTAKE_COUNT_KEY,
] as const

// a stable value for "nothing yet", so what is derived from it doesn't change on every render
const NONE: never[] = []

/**
 * The user's inbox, read again while something in it moves (being analysed, being filed after an
 * approval) and now and then while books wait for an approver
 */
export function useIntakeInbox() {
	const result = useGraphQL(intakeItemsQuery, INTAKE_ITEMS_KEY)
	const items = useFragment(intakeItemFragment, result.data?.intakeItems ?? NONE)

	const { refetch } = result
	const interval = pollInterval(items)
	useEffect(() => {
		if (!interval) return
		const timer = setInterval(() => void refetch(), interval)
		return () => clearInterval(timer)
	}, [interval, refetch])

	return { ...result, items }
}

/** What the viewer may upload, how much of it they have used, and whether uploads are open */
export function useIntakeLimits() {
	const result = useGraphQL(intakeUploadLimitsQuery, INTAKE_LIMITS_KEY)
	return { ...result, limits: result.data?.intakeUploadLimits }
}

/**
 * The books waiting for approval, a page at a time, oldest first. Read again every half minute
 * so a book sent while the page is open shows up
 */
export function usePendingIntake({
	libraryId,
	page,
	pageSize,
}: {
	libraryId?: string
	page: number
	pageSize: number
}) {
	const result = useGraphQL(
		pendingIntakeItemsQuery,
		[...PENDING_INTAKE_KEY, libraryId ?? 'all', page, pageSize],
		{ libraryId, page, pageSize },
		{ placeholderData: keepPreviousData, refetchInterval: 30_000 },
	)
	const items = useFragment(intakeItemFragment, result.data?.pendingIntakeItems.nodes ?? NONE)
	return { ...result, items, totalCount: result.data?.pendingIntakeItems.totalCount ?? 0 }
}

/** How many books wait for approval (for the badge of those who approve them) */
export function usePendingIntakeCount(enabled: boolean) {
	const result = useGraphQL(pendingIntakeCountQuery, PENDING_INTAKE_COUNT_KEY, undefined, {
		enabled,
		refetchInterval: 60_000,
	})
	return enabled ? (result.data?.pendingIntakeCount ?? 0) : 0
}

/**
 * The age limit of the person who uploaded a book, when the viewer may read it (the server owner
 * and those who read users do; for anybody else there is nobody to ask, and no warning)
 */
export function useUploaderAgeLimit(userId?: string): AgeLimit | null {
	const { data } = useGraphQL(
		intakeUploaderAgeQuery,
		['intakeUploaderAge', userId],
		{ userId: userId ?? '' },
		{ enabled: !!userId, retry: false, staleTime: 5 * 60_000 },
	)
	return data?.userById.ageRestriction ?? null
}

function useRefreshIntake() {
	const client = useQueryClient()
	return useCallback(
		() => Promise.all(INTAKE_KEYS.map((queryKey) => client.invalidateQueries({ queryKey }))),
		[client],
	)
}

type TagEntry = {
	item: Pick<IntakeItemData, 'id' | 'state' | 'decision'>
	genres: string[]
}

const plural = (count: number, one: string, many: string) =>
	count === 1 ? one : `${count} ${many}`

/**
 * Review / re-analyse / change library, then what is done with a book depending on who you are:
 * add it to the library (those who file books themselves), send it for approval or take it back
 * (those who may not), approve it or send it back with a note (those who approve). Each one
 * refreshes everything the intake shows afterwards.
 */
export function useIntakeActions() {
	const refresh = useRefreshIntake()
	const onError = (fallback: string) => (error: unknown) =>
		toast.error(fallback, { description: extractErrorMessage(error) })

	const review = useGraphQLMutation(intakeReviewMutation, {
		onError: onError('Could not save the change'),
		onSettled: refresh,
	})
	const reanalyse = useGraphQLMutation(intakeReanalyseMutation, {
		onError: onError('Could not analyse the book again'),
		onSettled: refresh,
	})
	const changeLibrary = useGraphQLMutation(intakeChangeLibraryMutation, {
		onError: onError('Could not move the book to that library'),
		onSettled: refresh,
	})
	const accept = useGraphQLMutation(intakeAcceptMutation, {
		onError: onError('Could not add the books'),
		onSettled: refresh,
		onSuccess: ({ intakeAccept }) => {
			const failed = intakeAccept.filter((item) => item.error)
			const filed = intakeAccept.length - failed.length
			if (filed) toast.success(filed === 1 ? 'Added 1 book' : `Added ${filed} books`)
			if (failed.length) {
				toast.error(
					failed.length === 1
						? 'One book could not be added'
						: `${failed.length} could not be added`,
					{ description: failed[0]?.error ?? undefined },
				)
			}
		},
	})
	const submit = useGraphQLMutation(intakeSubmitMutation, {
		onError: onError('Could not send the books for approval'),
		onSettled: refresh,
		onSuccess: ({ intakeSubmit }) =>
			toast.success(`Sent ${plural(intakeSubmit.length, '1 book', 'books')} for approval`, {
				description: 'You’ll see them here when they are added or sent back',
			}),
	})
	const withdraw = useGraphQLMutation(intakeWithdrawMutation, {
		onError: onError('Could not take the books back'),
		onSettled: refresh,
		onSuccess: ({ intakeWithdraw }) =>
			toast.success(`Took back ${plural(intakeWithdraw.length, '1 book', 'books')}`),
	})
	const approve = useGraphQLMutation(intakeApproveMutation, {
		onError: onError('Could not approve the books'),
		onSettled: refresh,
		onSuccess: ({ intakeApprove }) => {
			const failed = intakeApprove.filter((item) => item.error)
			const approved = intakeApprove.length - failed.length
			if (approved) {
				toast.success(`Approved ${plural(approved, '1 book', 'books')}`, {
					description: 'They are being added to the library',
				})
			}
			if (failed.length) {
				toast.error(
					failed.length === 1
						? 'One book could not be approved'
						: `${failed.length} could not be approved`,
					{ description: failed[0]?.error ?? undefined },
				)
			}
		},
	})
	const reject = useGraphQLMutation(intakeRejectMutation, {
		onError: onError('Could not send the book back'),
		onSettled: refresh,
		onSuccess: () => toast.success('Sent back with your note'),
	})
	const discard = useGraphQLMutation(intakeDiscardMutation, {
		onError: onError('Could not discard'),
		onSettled: refresh,
	})

	const reviewItem = useCallback(
		(id: string, input: IntakeReviewInput) => review.mutate({ id, input }),
		[review],
	)

	const { mutateAsync: reviewAsync } = review
	const { mutate: acceptMutate } = accept
	const { mutate: submitMutate } = submit

	/**
	 * Sends the tags the uploader confirmed, then what is to be done with the books. The server
	 * writes the tags of its decision and only takes books that are ready, so books whose tags
	 * differ (weak guesses left unticked) or that were flagged for checking get their tags sent
	 * first (a review of a book with nothing blocking makes it ready).
	 */
	const withConfirmedTags = useCallback(
		async (entries: TagEntry[], then: (ids: string[]) => void) => {
			const needReview = entries.filter(
				({ item, genres }) => item.state === IntakeState.NeedsReview || genresDiffer(item, genres),
			)
			try {
				for (const { item, genres } of needReview) {
					await reviewAsync({ id: item.id, input: { genres } })
				}
			} catch {
				return
			}
			if (entries.length) then(entries.map(({ item }) => item.id))
		},
		[reviewAsync],
	)

	const acceptWithGenres = useCallback(
		(entries: TagEntry[]) => withConfirmedTags(entries, (ids) => acceptMutate({ ids })),
		[withConfirmedTags, acceptMutate],
	)
	const submitWithGenres = useCallback(
		(entries: TagEntry[]) => withConfirmedTags(entries, (ids) => submitMutate({ ids })),
		[withConfirmedTags, submitMutate],
	)

	const acceptItem = useCallback(
		(item: TagEntry['item'], genres: string[]) => acceptWithGenres([{ genres, item }]),
		[acceptWithGenres],
	)
	const submitItem = useCallback(
		(item: TagEntry['item'], genres: string[]) => submitWithGenres([{ genres, item }]),
		[submitWithGenres],
	)

	return {
		acceptItem,
		acceptWithGenres,
		approve: (ids: string[]) => approve.mutate({ ids }),
		changeLibrary: (id: string, libraryId: string) => changeLibrary.mutate({ id, libraryId }),
		discard: (ids: string[]) => discard.mutate({ ids }),
		isAccepting: accept.isPending,
		isApproving: approve.isPending,
		isChangingLibrary: changeLibrary.isPending,
		isRejecting: reject.isPending,
		isReviewing: review.isPending,
		isSubmitting: submit.isPending,
		isWithdrawing: withdraw.isPending,
		reanalyse: (id: string, online?: boolean) => reanalyse.mutate({ id, online }),
		reject: (id: string, note: string, onDone?: () => void) =>
			reject.mutate({ id, note }, { onSuccess: onDone }),
		reviewItem,
		submitItem,
		submitWithGenres,
		withdraw: (ids: string[]) => withdraw.mutate({ ids }),
	}
}

export type UploadStatus = 'queued' | 'uploading' | 'done' | 'error'

export type UploadEntry = UploadBatch & {
	status: UploadStatus
	progress: number
	error?: string
}

/** Those who file books themselves have the room for two uploads at a time; everybody else's is one */
const PARALLEL_UPLOADS_FOR_APPROVERS = 2

const modeParam = (mode: IntakeUploadMode) => mode.toLowerCase() as 'auto' | 'books' | 'series'

/**
 * Uploads picked files into the inbox of a library over `POST /api/v2/intake/upload`: each loose
 * book is its own request and each folder one request, one at a time (two for those who file
 * books themselves), with progress per request. Before a request is sent the session is checked
 * and the upload is held against the server's limits, so a refusal that is certain comes at once
 * and in words, not after the bytes have gone.
 */
export function useIntakeUploader(libraryId?: string, limits?: IntakeLimitsData) {
	const { sdk } = useSDK()
	const refresh = useRefreshIntake()
	const [entries, setEntries] = useState<UploadEntry[]>([])
	const active = useRef(0)
	const queue = useRef<UploadEntry[]>([])
	// bumped when an upload finishes or files are added, to start the next uploads
	const [tick, setTick] = useState(0)
	// the latest limits, for uploads that start later than the render that queued them
	const limitsRef = useRef(limits)
	useEffect(() => {
		limitsRef.current = limits
	}, [limits])

	const update = useCallback((key: string, patch: Partial<UploadEntry>) => {
		setEntries((prev) => prev.map((entry) => (entry.key === key ? { ...entry, ...patch } : entry)))
	}, [])

	const send = useCallback(
		async (entry: UploadEntry, into: string) => {
			try {
				// a session that has ended refuses the upload after the whole body is sent
				await sdk.auth.me()
			} catch (error) {
				throw intakeUploadErrorOf(error)
			}
			await sdk.intake.upload({
				files: entry.files,
				libraryId: into,
				mode: modeParam(entry.mode),
				onUploadProgress: ({ loaded, total }) =>
					update(entry.key, { progress: uploadPercent(loaded, total) }),
				relativePaths: entry.relativePaths,
			})
		},
		[sdk, update],
	)

	const pump = useCallback(() => {
		const parallel = limitsRef.current?.canFileDirectly ? PARALLEL_UPLOADS_FOR_APPROVERS : 1
		while (active.current < parallel && queue.current.length) {
			const next = queue.current.shift()
			if (!next || !libraryId) break
			const known = limitsRef.current
			const problem = known ? uploadProblem(next, known) : undefined
			if (problem) {
				update(next.key, { error: problem, status: 'error' })
				continue
			}
			active.current += 1
			update(next.key, { status: 'uploading' })
			send(next, libraryId)
				.then(() => update(next.key, { progress: 100, status: 'done' }))
				.catch((error: unknown) =>
					update(next.key, {
						error: intakeUploadErrorOf(error).message,
						status: 'error',
					}),
				)
				.finally(() => {
					active.current -= 1
					void refresh()
					setTick((t) => t + 1)
				})
		}
	}, [libraryId, send, update, refresh])

	const add = useCallback(
		(files: PendingFile[], folderMode: IntakeUploadMode = IntakeUploadMode.Auto) => {
			const batches = planUploads(files).map<UploadEntry>((batch) => ({
				...batch,
				// keys stay unique when the same file is picked twice
				key: `${batch.key}:${Date.now()}:${Math.random().toString(36).slice(2, 7)}`,
				mode: batch.isFolder ? folderMode : batch.mode,
				progress: 0,
				status: 'queued',
			}))
			if (!batches.length) {
				toast.error('No books found', {
					description: 'Pick EPUB, PDF, CBZ or CBR files, folders of them, or a .zip',
				})
				return
			}
			setEntries((prev) => [...batches, ...prev])
			queue.current.push(...batches)
			setTick((t) => t + 1)
		},
		[],
	)

	const clearFinished = useCallback(
		() => setEntries((prev) => prev.filter((entry) => entry.status !== 'done')),
		[],
	)

	// start queued uploads (new files, a finished upload, or the library becoming known)
	useEffect(() => {
		pump()
	}, [tick, pump])

	return { add, clearFinished, entries }
}
