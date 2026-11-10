import type { AxiosProgressEvent } from 'axios'

import { APIBase } from '../base'
import { ClassQueryKeys } from './types'
import { createRouteURLHandler, isAxiosError } from './utils'

/**
 * The root route for the "Add books" intake API
 */
const INTAKE_ROUTE = '/intake'
const intakeURL = createRouteURLHandler(INTAKE_ROUTE)

/** How an upload of several files is grouped into inbox items (the server reads it in any case) */
export type IntakeUploadMode = 'auto' | 'books' | 'series'

export type IntakeUploadParams = {
	libraryId: string
	files: File[]
	/**
	 * The path of each file inside an uploaded folder, same order as `files`. Without it a file
	 * goes by its own name
	 */
	relativePaths?: string[]
	mode?: IntakeUploadMode
	onUploadProgress?: (event: AxiosProgressEvent) => void
	signal?: AbortSignal
}

export type IntakeUploadedItem = {
	id: string
	state: string
	name: string
	kind: string
	libraryId: string
}

export type IntakeUploadResponse = {
	items: IntakeUploadedItem[]
}

/**
 * Why the server refused an upload. `status` is the HTTP status that says it: 401 the session is
 * over, 403 not allowed (or uploads are off), 404 no such library, 408 a file stalled, 411 the
 * request didn't say how big it is, 413 too big or no room left, 415 not a book Stump reads, 429
 * too many files or items, 503 uploads are closed or the disk is full
 */
export class IntakeUploadError extends Error {
	readonly status?: number

	constructor(message: string, status?: number) {
		super(message)
		this.name = 'IntakeUploadError'
		this.status = status
	}
}

/** What to say for a status when the answer carries no message of the server's (a proxy's page) */
const STATUS_MESSAGES: Record<number, string> = {
	401: 'Your session has ended: sign in again to upload',
	403: 'You can’t add books here',
	404: 'That library is not available',
	408: 'The upload stopped arriving',
	413: 'The upload is larger than the server accepts',
	429: 'Too many requests: wait a moment and try again',
	502: 'The server could not be reached',
	503: 'The server can’t take uploads right now',
	504: 'The server took too long to answer',
}

/** The message of a refusal (`{"status", "message"}`), else one that fits the status */
export function intakeUploadErrorOf(error: unknown): IntakeUploadError {
	if (error instanceof IntakeUploadError) return error
	if (isAxiosError(error)) {
		const status = error.response?.status
		const data = error.response?.data as { message?: unknown } | undefined
		if (typeof data?.message === 'string' && data.message) {
			return new IntakeUploadError(data.message, status)
		}
		if (status !== undefined) {
			return new IntakeUploadError(
				STATUS_MESSAGES[status] ?? `The upload failed (${status})`,
				status,
			)
		}
		// no answer at all: the connection broke, or the browser gave up
		return new IntakeUploadError(
			error.code === 'ERR_CANCELED' ? 'The upload was cancelled' : 'The connection was lost',
		)
	}
	return new IntakeUploadError(error instanceof Error ? error.message : 'The upload failed')
}

/**
 * The "Add books" intake: the way books arrive in the user's inbox. The body is streamed to disk
 * by the server within the user's limits (`POST /api/v2/intake/upload`), which is why it is not a
 * GraphQL upload: that one is held whole before anything is checked
 */
export class IntakeAPI extends APIBase {
	/**
	 * Uploads the files into the user's inbox for a library. Each book, or each folder or .zip of
	 * volumes, becomes an item that is analysed right away: read `intakeItems` to see it
	 */
	async upload({
		libraryId,
		files,
		relativePaths,
		mode = 'auto',
		onUploadProgress,
		signal,
	}: IntakeUploadParams): Promise<IntakeUploadResponse> {
		const body = new FormData()
		files.forEach((file, index) => {
			// the part's file name is how the server tells where in a folder a file was
			body.append('files', file, relativePaths?.[index] || file.name)
		})

		try {
			const { data } = await this.api.axios.post<IntakeUploadResponse>(
				intakeURL('/upload', { libraryId, mode }),
				body,
				{
					// the content type is left to the browser: it adds the boundary
					headers: { ...this.api.headers },
					onUploadProgress,
					signal,
				},
			)
			return data
		} catch (error) {
			throw intakeUploadErrorOf(error)
		}
	}

	/**
	 * The query keys for the intake API, used for query caching on a client (e.g. react-query)
	 */
	get keys(): ClassQueryKeys<InstanceType<typeof IntakeAPI>> {
		return {
			upload: 'intake.upload',
		}
	}
}
