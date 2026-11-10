import type { AxiosInstance } from 'axios'

import { Api } from '../../api'
import { IntakeAPI, IntakeUploadError, intakeUploadErrorOf } from '../intake-api'

const file = (name: string, size = 3) => new File([new Uint8Array(size)], name)

function setup(post: ReturnType<typeof vi.fn>) {
	const api = {
		axios: { post } as unknown as AxiosInstance,
		headers: { Authorization: 'Bearer token' },
		serviceURL: 'http://localhost/api/v2',
	} as unknown as Api
	return new IntakeAPI(api)
}

/** What axios throws for an answer with a status */
const refusal = (status: number, data: unknown, headers?: Record<string, string>) => ({
	isAxiosError: true,
	response: { data, headers, status },
})

describe('IntakeAPI.upload', () => {
	it('posts the files to the intake route for the library, in the mode asked for', async () => {
		const post = vi.fn().mockResolvedValue({
			data: {
				items: [{ id: 'i1', kind: 'BOOK', libraryId: 'lib', name: 'a.epub', state: 'ANALYSING' }],
			},
		})
		const intake = setup(post)

		const response = await intake.upload({
			files: [file('a.epub')],
			libraryId: 'lib 1',
			mode: 'books',
		})

		expect(response.items[0]?.id).toBe('i1')
		expect(post).toHaveBeenCalledTimes(1)
		const [url, body, config] = post.mock.calls[0] ?? []
		expect(url).toContain('/intake/upload')
		expect(url).toContain('libraryId=lib+1')
		expect(url).toContain('mode=books')
		expect(body).toBeInstanceOf(FormData)
		expect(config.headers).toEqual({ Authorization: 'Bearer token' })
	})

	it('names each part by its path in the folder, or by the file name', async () => {
		const post = vi.fn().mockResolvedValue({ data: { items: [] } })

		await setup(post).upload({
			files: [file('v1.cbz'), file('cover.jpg'), file('loose.epub')],
			libraryId: 'lib',
			relativePaths: ['Yotsuba/v1.cbz', 'Yotsuba/extras/cover.jpg'],
		})

		const body = post.mock.calls[0]?.[1] as FormData
		const names = body.getAll('files').map((part) => (part as File).name)
		expect(names).toEqual(['Yotsuba/v1.cbz', 'Yotsuba/extras/cover.jpg', 'loose.epub'])
	})

	it('asks the server to group the files by itself unless told otherwise', async () => {
		const post = vi.fn().mockResolvedValue({ data: { items: [] } })

		await setup(post).upload({ files: [file('a.epub')], libraryId: 'lib' })

		expect(post.mock.calls[0]?.[0]).toContain('mode=auto')
	})

	it('hands the progress and the abort signal to the request', async () => {
		const post = vi.fn().mockResolvedValue({ data: { items: [] } })
		const onUploadProgress = vi.fn()
		const signal = new AbortController().signal

		await setup(post).upload({
			files: [file('a.epub')],
			libraryId: 'lib',
			onUploadProgress,
			signal,
		})

		const config = post.mock.calls[0]?.[2]
		expect(config.onUploadProgress).toBe(onUploadProgress)
		expect(config.signal).toBe(signal)
	})

	it('says what the server said when it refuses an upload', async () => {
		const post = vi
			.fn()
			.mockRejectedValue(
				refusal(413, { message: 'There is no room left in your inbox', status: 413 }),
			)

		const error = await setup(post)
			.upload({ files: [file('a.epub')], libraryId: 'lib' })
			.catch((e: unknown) => e)

		expect(error).toBeInstanceOf(IntakeUploadError)
		expect((error as IntakeUploadError).message).toBe('There is no room left in your inbox')
		expect((error as IntakeUploadError).status).toBe(413)
	})
})

describe('intakeUploadErrorOf', () => {
	it('falls back to a sentence for the status when a proxy answers with a page of its own', () => {
		const error = intakeUploadErrorOf(refusal(413, '<html>413 Request Entity Too Large</html>'))

		expect(error.status).toBe(413)
		expect(error.message).toBe('The upload is larger than the server accepts')
	})

	it('tells an ended session from the other refusals', () => {
		expect(intakeUploadErrorOf(refusal(401, undefined)).message).toMatch(/sign in again/)
	})

	it('names a status it has no sentence for', () => {
		expect(intakeUploadErrorOf(refusal(418, undefined)).message).toBe('The upload failed (418)')
	})

	it('says so when there was no answer at all', () => {
		const lost = intakeUploadErrorOf({ code: 'ERR_NETWORK', isAxiosError: true })
		const cancelled = intakeUploadErrorOf({ code: 'ERR_CANCELED', isAxiosError: true })

		expect(lost.message).toBe('The connection was lost')
		expect(cancelled.message).toBe('The upload was cancelled')
		expect(lost.status).toBeUndefined()
	})

	it('knows when it was Stump that said no and when it was a proxy in front of it', () => {
		const stump = intakeUploadErrorOf(
			refusal(429, { message: 'An upload can hold 50 files at most', status: 429 }),
		)
		const proxy = intakeUploadErrorOf(refusal(429, '<html>429 Too Many Requests</html>'))

		expect(stump.fromServer).toBe(true)
		expect(proxy.fromServer).toBe(false)
		// only a proxy's rate limit lifts by itself
		expect(stump.throttledByProxy).toBe(false)
		expect(proxy.throttledByProxy).toBe(true)
		expect(intakeUploadErrorOf(refusal(413, '<html>too big</html>')).throttledByProxy).toBe(false)
		expect(intakeUploadErrorOf({ code: 'ERR_NETWORK', isAxiosError: true }).throttledByProxy).toBe(
			false,
		)
	})

	it('reads how long a proxy asks to wait, when it says in seconds', () => {
		const asked = intakeUploadErrorOf(refusal(429, undefined, { 'retry-after': '20' }))
		const date = intakeUploadErrorOf(
			refusal(429, undefined, { 'retry-after': 'Wed, 21 Oct 2026 07:28:00 GMT' }),
		)
		const silent = intakeUploadErrorOf(refusal(429, undefined))

		expect(asked.retryAfterSecs).toBe(20)
		// a date is not read: the uploader's own pause is used
		expect(date.retryAfterSecs).toBeUndefined()
		expect(silent.retryAfterSecs).toBeUndefined()
	})

	it('keeps an error that is already one of ours', () => {
		const ours = new IntakeUploadError('Nope', 403)

		expect(intakeUploadErrorOf(ours)).toBe(ours)
	})
})

describe('IntakeAPI.keys', () => {
	it('exposes an upload query key', () => {
		expect(setup(vi.fn()).keys.upload).toBe('intake.upload')
	})
})
