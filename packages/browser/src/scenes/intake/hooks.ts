import { useGraphQL, useGraphQLMutation, useSDK } from '@stump/client'
import { extractErrorMessage, IntakeReviewInput, IntakeUploadMode } from '@stump/graphql'
import { useQueryClient } from '@tanstack/react-query'
import { useCallback, useEffect, useRef, useState } from 'react'
import { toast } from 'sonner'

import {
	intakeAcceptMutation,
	intakeDiscardMutation,
	intakeItemsQuery,
	intakeReanalyseMutation,
	intakeReviewMutation,
	intakeUploadMutation,
} from './queries'
import {
	type PendingFile,
	planUploads,
	shouldPoll,
	type UploadBatch,
	uploadPercent,
} from './reviewState'

export const INTAKE_ITEMS_KEY = ['intakeItems'] as const

/** The user's inbox, polled while something is being analysed or scanned */
export function useIntakeInbox() {
	const result = useGraphQL(intakeItemsQuery, INTAKE_ITEMS_KEY, undefined, {
		refetchInterval: (query) => (shouldPoll(query.state.data?.intakeItems ?? []) ? 1500 : false),
	})
	return { ...result, items: result.data?.intakeItems ?? [] }
}

function useRefreshInbox() {
	const client = useQueryClient()
	return useCallback(() => client.invalidateQueries({ queryKey: INTAKE_ITEMS_KEY }), [client])
}

/** Review / re-analyse / accept / discard, each refreshing the inbox afterwards */
export function useIntakeActions() {
	const refresh = useRefreshInbox()
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
	const discard = useGraphQLMutation(intakeDiscardMutation, {
		onError: onError('Could not discard'),
		onSettled: refresh,
	})

	const reviewItem = useCallback(
		(id: string, input: IntakeReviewInput) => review.mutate({ id, input }),
		[review],
	)

	return {
		accept: (ids: string[]) => accept.mutate({ ids }),
		discard: (ids: string[]) => discard.mutate({ ids }),
		isAccepting: accept.isPending,
		isReviewing: review.isPending,
		reanalyse: (id: string, online?: boolean) => reanalyse.mutate({ id, online }),
		reviewItem,
	}
}

export type UploadStatus = 'queued' | 'uploading' | 'done' | 'error'

export type UploadEntry = UploadBatch & {
	status: UploadStatus
	progress: number
	error?: string
}

const MAX_PARALLEL_UPLOADS = 2

/**
 * Uploads picked files into the inbox of a library: each loose book is its own request and
 * each folder one request, two at a time, with progress per request.
 */
export function useIntakeUploader(libraryId?: string) {
	const { sdk } = useSDK()
	const refresh = useRefreshInbox()
	const [entries, setEntries] = useState<UploadEntry[]>([])
	const active = useRef(0)
	const queue = useRef<UploadEntry[]>([])
	// bumped when an upload finishes or files are added, to start the next uploads
	const [tick, setTick] = useState(0)

	const update = useCallback((key: string, patch: Partial<UploadEntry>) => {
		setEntries((prev) => prev.map((entry) => (entry.key === key ? { ...entry, ...patch } : entry)))
	}, [])

	const pump = useCallback(() => {
		while (active.current < MAX_PARALLEL_UPLOADS && queue.current.length) {
			const next = queue.current.shift()
			if (!next || !libraryId) break
			active.current += 1
			update(next.key, { status: 'uploading' })
			sdk
				.executeUpload(
					intakeUploadMutation,
					{
						files: next.files,
						libraryId,
						mode: next.mode,
						relativePaths: next.relativePaths,
					},
					{
						onUploadProgress: ({ loaded, total }) =>
							update(next.key, { progress: uploadPercent(loaded, total) }),
					},
				)
				.then(() => update(next.key, { progress: 100, status: 'done' }))
				.catch((error: unknown) =>
					update(next.key, {
						error: extractErrorMessage(error, 'Upload failed'),
						status: 'error',
					}),
				)
				.finally(() => {
					active.current -= 1
					refresh()
					setTick((t) => t + 1)
				})
		}
	}, [libraryId, sdk, update, refresh])

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
