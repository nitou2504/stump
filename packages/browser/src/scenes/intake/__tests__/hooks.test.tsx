import { IntakeState, IntakeUploadMode } from '@stump/graphql'
import { IntakeUploadError } from '@stump/sdk'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act, renderHook, waitFor } from '@testing-library/react'
import type { PropsWithChildren } from 'react'

import {
	PROXY_RETRIES,
	PROXY_RETRY_MAX_SECS,
	PROXY_RETRY_MIN_SECS,
	useIntakeActions,
	useIntakeInbox,
	useIntakeUploader,
} from '../hooks'
import {
	intakeAcceptMutation,
	intakeApproveMutation,
	type IntakeLimitsData,
	intakeReviewMutation,
	intakeSubmitMutation,
} from '../queries'
import { POLL_FAST_MS, POLL_SLOW_MS } from '../reviewState'
import { item } from './fixtures'

const sdk = vi.hoisted(() => ({ auth: { me: vi.fn() }, intake: { upload: vi.fn() } }))
const inbox = vi.hoisted(() => ({ data: undefined as unknown, refetch: vi.fn() }))
// one set of stand-ins per mutation (by its document), the same on every render
const mutations = vi.hoisted(
	() =>
		new Map<
			unknown,
			{
				isPending: boolean
				mutate: ReturnType<typeof vi.fn>
				mutateAsync: ReturnType<typeof vi.fn>
			}
		>(),
)
const mutationOf = (document: unknown) => {
	let entry = mutations.get(document)
	if (!entry) {
		entry = { isPending: false, mutate: vi.fn(), mutateAsync: vi.fn().mockResolvedValue({}) }
		mutations.set(document, entry)
	}
	return entry
}
vi.mock('@stump/client', async (importOriginal) => ({
	...(await importOriginal<typeof import('@stump/client')>()),
	useGraphQL: () => ({ data: inbox.data, refetch: inbox.refetch }),
	useGraphQLMutation: (document: unknown) => mutationOf(document),
	useSDK: () => ({ sdk }),
}))
const toast = vi.hoisted(() => ({ error: vi.fn(), success: vi.fn() }))
vi.mock('sonner', () => ({ toast }))

const MB = 1024 * 1024

/** A file of a size that costs no memory */
const file = (name: string, mb = 1) => {
	const f = new File([''], name)
	Object.defineProperty(f, 'size', { value: mb * MB })
	return f
}

const limits = (overrides: Partial<IntakeLimitsData> = {}): IntakeLimitsData => ({
	canFileDirectly: false,
	closedReason: null,
	maxFileBytes: 100 * MB,
	maxFilesPerRequest: 10,
	maxItems: 100,
	maxRequestBytes: 300 * MB,
	openItems: 0,
	quotaApplies: true,
	quotaBytes: 2048 * MB,
	uploadsOpen: true,
	usedBytes: 0,
	...overrides,
})

const deferred = () => {
	let resolve!: (value: unknown) => void
	let reject!: (error: unknown) => void
	const promise = new Promise((res, rej) => {
		resolve = res
		reject = rej
	})
	return { promise, reject, resolve }
}

const wrapper = ({ children }: PropsWithChildren) => (
	<QueryClientProvider client={new QueryClient()}>{children}</QueryClientProvider>
)

const setup = (known: IntakeLimitsData = limits()) =>
	renderHook(() => useIntakeUploader('lib', known), { wrapper })

/** The limits have not come from the server yet */
const setupWithoutLimits = () => renderHook(() => useIntakeUploader('lib'), { wrapper })

describe('useIntakeUploader', () => {
	beforeEach(() => {
		vi.clearAllMocks()
		sdk.auth.me.mockResolvedValue({})
		sdk.intake.upload.mockResolvedValue({ items: [] })
	})

	it('checks the session, then uploads the book into the library over the intake route', async () => {
		const { result } = setup()

		act(() => result.current.add([{ file: file('a.epub') }]))

		await waitFor(() => expect(result.current.entries[0]?.status).toBe('done'))
		expect(sdk.auth.me).toHaveBeenCalledTimes(1)
		expect(sdk.intake.upload).toHaveBeenCalledTimes(1)
		const call = sdk.intake.upload.mock.calls[0]?.[0]
		expect(call).toMatchObject({ libraryId: 'lib', mode: 'auto' })
		expect(call.files).toHaveLength(1)
		// the session was asked about before a byte was sent
		expect(sdk.auth.me.mock.invocationCallOrder[0]).toBeLessThan(
			sdk.intake.upload.mock.invocationCallOrder[0] ?? 0,
		)
		expect(result.current.entries[0]?.progress).toBe(100)
	})

	it('sends a picked folder as one request with the path of each file, in the mode chosen', async () => {
		const { result } = setup()

		act(() =>
			result.current.add(
				[
					{ file: file('v1.cbz'), relativePath: 'Yotsuba/v1.cbz' },
					{ file: file('v2.cbz'), relativePath: 'Yotsuba/v2.cbz' },
				],
				IntakeUploadMode.Books,
			),
		)

		await waitFor(() => expect(result.current.entries[0]?.status).toBe('done'))
		expect(sdk.intake.upload).toHaveBeenCalledTimes(1)
		expect(sdk.intake.upload.mock.calls[0]?.[0]).toMatchObject({
			mode: 'books',
			relativePaths: ['Yotsuba/v1.cbz', 'Yotsuba/v2.cbz'],
		})
	})

	it('uploads one request at a time for somebody who needs approval', async () => {
		const first = deferred()
		const second = deferred()
		sdk.intake.upload.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise)
		const { result } = setup(limits({ canFileDirectly: false }))

		act(() => result.current.add([{ file: file('a.epub') }, { file: file('b.epub') }]))

		await waitFor(() => expect(sdk.intake.upload).toHaveBeenCalledTimes(1))
		// the second waits for the first, however long that takes
		await act(async () => {
			await Promise.resolve()
		})
		expect(sdk.intake.upload).toHaveBeenCalledTimes(1)
		expect(result.current.entries.filter((e) => e.status === 'queued')).toHaveLength(1)

		await act(async () => first.resolve({ items: [] }))
		await waitFor(() => expect(sdk.intake.upload).toHaveBeenCalledTimes(2))
		await act(async () => second.resolve({ items: [] }))
		await waitFor(() => expect(result.current.entries.every((e) => e.status === 'done')).toBe(true))
	})

	it('uploads two at a time for somebody who files books themselves', async () => {
		const pending = [deferred(), deferred(), deferred()]
		pending.forEach((p) => sdk.intake.upload.mockReturnValueOnce(p.promise))
		const { result } = setup(limits({ canFileDirectly: true, quotaApplies: false }))

		act(() =>
			result.current.add([
				{ file: file('a.epub') },
				{ file: file('b.epub') },
				{ file: file('c.epub') },
			]),
		)

		await waitFor(() => expect(sdk.intake.upload).toHaveBeenCalledTimes(2))
		await act(async () => {
			await Promise.resolve()
		})
		expect(sdk.intake.upload).toHaveBeenCalledTimes(2)
		await act(async () => pending[0]?.resolve({ items: [] }))
		await waitFor(() => expect(sdk.intake.upload).toHaveBeenCalledTimes(3))
	})

	it('refuses a file that is too big before sending a byte, and says why', async () => {
		const { result } = setup(limits({ maxFileBytes: 50 * MB }))

		act(() => result.current.add([{ file: file('huge.cbz', 80) }, { file: file('ok.epub') }]))

		await waitFor(() =>
			expect(result.current.entries.map((e) => e.status).sort()).toEqual(['done', 'error']),
		)
		const refused = result.current.entries.find((e) => e.status === 'error')
		expect(refused?.error).toContain('huge.cbz')
		expect(refused?.error).toContain('50.0 MB')
		// only the other book went: no request, and no session check, for the refused one
		expect(sdk.intake.upload).toHaveBeenCalledTimes(1)
		expect(sdk.auth.me).toHaveBeenCalledTimes(1)
	})

	it('refuses a book that does not fit the room left in the inbox', async () => {
		const { result } = setup(limits({ quotaBytes: 100 * MB, usedBytes: 95 * MB }))

		act(() => result.current.add([{ file: file('a.epub', 10) }]))

		await waitFor(() => expect(result.current.entries[0]?.status).toBe('error'))
		expect(result.current.entries[0]?.error).toContain('not enough room')
		expect(sdk.intake.upload).not.toHaveBeenCalled()
	})

	it('refuses everything while uploads are closed, with the reason the server gave', async () => {
		const { result } = setup(
			limits({ closedReason: 'Uploads are paused for the night', uploadsOpen: false }),
		)

		act(() => result.current.add([{ file: file('a.epub') }]))

		await waitFor(() => expect(result.current.entries[0]?.status).toBe('error'))
		expect(result.current.entries[0]?.error).toBe('Uploads are paused for the night')
		expect(sdk.intake.upload).not.toHaveBeenCalled()
	})

	it('does not send a book when the session has ended, and says to sign in again', async () => {
		sdk.auth.me.mockRejectedValue({
			isAxiosError: true,
			response: { data: undefined, status: 401 },
		})
		const { result } = setup()

		act(() => result.current.add([{ file: file('a.epub') }]))

		await waitFor(() => expect(result.current.entries[0]?.status).toBe('error'))
		expect(result.current.entries[0]?.error).toMatch(/sign in again/)
		expect(sdk.intake.upload).not.toHaveBeenCalled()
	})

	it('shows what the server said when it refuses an upload, and goes on with the next', async () => {
		sdk.intake.upload
			.mockRejectedValueOnce(new IntakeUploadError('There is no room left in your inbox', 413))
			.mockResolvedValueOnce({ items: [] })
		const { result } = setup()

		act(() => result.current.add([{ file: file('a.epub') }, { file: file('b.epub') }]))

		await waitFor(() => expect(sdk.intake.upload).toHaveBeenCalledTimes(2))
		await waitFor(() =>
			expect(result.current.entries.map((e) => e.status).sort()).toEqual(['done', 'error']),
		)
		expect(result.current.entries.find((e) => e.status === 'error')?.error).toBe(
			'There is no room left in your inbox',
		)
	})

	it('sends without a pre-check while the limits are not known yet: the server decides', async () => {
		const { result } = setupWithoutLimits()

		act(() => result.current.add([{ file: file('huge.cbz', 500) }]))

		await waitFor(() => expect(sdk.intake.upload).toHaveBeenCalledTimes(1))
	})

	it('says so when nothing that was picked is a book', () => {
		const { result } = setup()

		act(() => result.current.add([{ file: file('notes.txt') }]))

		expect(toast.error).toHaveBeenCalledWith('No books found', expect.anything())
		expect(result.current.entries).toHaveLength(0)
		expect(sdk.intake.upload).not.toHaveBeenCalled()
	})

	it('says that Stump can’t read a .cb7, instead of leaving it out without a word', () => {
		const { result } = setup()

		act(() => result.current.add([{ file: file('manga.cb7') }]))

		expect(toast.error).toHaveBeenCalledTimes(1)
		expect(toast.error).toHaveBeenCalledWith('Stump can’t read .cb7 files', {
			description: expect.stringContaining('It reads EPUB, PDF, CBZ and CBR'),
		})
		expect(result.current.entries).toHaveLength(0)
		expect(sdk.intake.upload).not.toHaveBeenCalled()
	})

	it('adds the books of a pick that had files Stump can’t read, and says what was left out', async () => {
		const { result } = setup()

		act(() =>
			result.current.add([
				{ file: file('a.epub') },
				{ file: file('b.7z') },
				{ file: file('c.cbt'), relativePath: 'Folder/c.cbt' },
				{ file: file('d.7z') },
			]),
		)

		// each kind once, and the books that can be read still go
		expect(toast.error).toHaveBeenCalledWith('Stump can’t read .7z and .cbt files', {
			description: 'The other books are added',
		})
		await waitFor(() => expect(result.current.entries.map((e) => e.label)).toEqual(['a.epub']))
		await waitFor(() => expect(sdk.intake.upload).toHaveBeenCalledTimes(1))
	})

	it('forgets the finished uploads and keeps the rest', async () => {
		const { result } = setup(limits({ maxFileBytes: 50 * MB }))
		act(() => result.current.add([{ file: file('huge.cbz', 80) }, { file: file('ok.epub') }]))
		await waitFor(() => expect(result.current.entries.some((e) => e.status === 'done')).toBe(true))

		act(() => result.current.clearFinished())

		expect(result.current.entries.map((e) => e.status)).toEqual(['error'])
	})
})

describe('useIntakeUploader behind a proxy that limits how fast requests come', () => {
	/** What the proxy answers: a 429 that carries no message of Stump's */
	const slowDown = (retryAfterSecs?: number) =>
		new IntakeUploadError('Too many requests: wait a moment and try again', 429, { retryAfterSecs })
	const statuses = (result: ReturnType<typeof setup>['result']) =>
		result.current.entries.map((e) => e.status)
	/** Lets the time pass, and the promises that wait for it */
	const pass = (ms: number) => act(() => vi.advanceTimersByTimeAsync(ms))
	/** Lets the chain of promises of an upload (the session, the request, what follows) run out */
	const settle = async () => {
		for (let i = 0; i < 8; i += 1) await pass(0)
	}

	beforeEach(() => {
		vi.clearAllMocks()
		vi.useFakeTimers()
		sdk.auth.me.mockResolvedValue({})
		sdk.intake.upload.mockResolvedValue({ items: [] })
	})
	afterEach(() => vi.useRealTimers())

	it('puts the book back and tries again after a pause, one at a time and in the same order', async () => {
		sdk.intake.upload.mockRejectedValueOnce(slowDown())
		const { result } = setup()

		act(() => result.current.add([{ file: file('a.epub') }, { file: file('b.epub') }]))
		await pass(0)

		// a was refused: it waits, with the reason, and b does not go in its place
		expect(sdk.intake.upload).toHaveBeenCalledTimes(1)
		const first = result.current.entries.find((e) => e.label === 'a.epub')
		expect(first?.status).toBe('queued')
		expect(first?.note).toBe(`The server is busy: trying again in ${PROXY_RETRY_MIN_SECS} seconds`)
		expect(first?.error).toBeUndefined()

		await pass(PROXY_RETRY_MIN_SECS * 1000 - 100)
		expect(sdk.intake.upload).toHaveBeenCalledTimes(1)
		await pass(200)

		// a goes first again, then b
		await settle()
		expect(statuses(result)).toEqual(['done', 'done'])
		expect(sdk.intake.upload).toHaveBeenCalledTimes(3)
		const sent = sdk.intake.upload.mock.calls.map(([call]) => call.files[0].name)
		expect(sent).toEqual(['a.epub', 'a.epub', 'b.epub'])
		expect(result.current.entries.every((e) => e.note === undefined)).toBe(true)
	})

	it('waits as long as the answer asks, up to a minute', async () => {
		sdk.intake.upload.mockRejectedValueOnce(slowDown(20)).mockRejectedValueOnce(slowDown(3600))
		const { result } = setup()

		act(() => result.current.add([{ file: file('a.epub') }]))
		await pass(0)
		expect(result.current.entries[0]?.note).toContain('20 seconds')
		await pass(19_000)
		expect(sdk.intake.upload).toHaveBeenCalledTimes(1)
		await pass(1_100)
		expect(sdk.intake.upload).toHaveBeenCalledTimes(2)

		// an hour is more than anybody waits at a page: a minute, then again
		expect(result.current.entries[0]?.note).toContain(`${PROXY_RETRY_MAX_SECS} seconds`)
		await pass(PROXY_RETRY_MAX_SECS * 1000 + 100)
		await settle()
		expect(statuses(result)).toEqual(['done'])
	})

	it('gives up after a few tries and says what the proxy said', async () => {
		sdk.intake.upload.mockRejectedValue(slowDown())
		const { result } = setup()

		act(() => result.current.add([{ file: file('a.epub') }]))
		for (let i = 0; i <= PROXY_RETRIES; i += 1) await pass(PROXY_RETRY_MIN_SECS * 1000 + 100)

		expect(sdk.intake.upload).toHaveBeenCalledTimes(PROXY_RETRIES + 1)
		expect(statuses(result)).toEqual(['error'])
		expect(result.current.entries[0]?.error).toBe('Too many requests: wait a moment and try again')
	})

	it('does not try again what Stump itself refused, even with a 429', async () => {
		sdk.intake.upload.mockRejectedValue(
			new IntakeUploadError('An upload can hold 2 files at most', 429, { fromServer: true }),
		)
		const { result } = setup()

		act(() => result.current.add([{ file: file('a.epub') }]))
		await pass(0)

		expect(statuses(result)).toEqual(['error'])
		expect(result.current.entries[0]?.error).toBe('An upload can hold 2 files at most')
		await pass(PROXY_RETRY_MAX_SECS * 1000)
		expect(sdk.intake.upload).toHaveBeenCalledTimes(1)
	})

	it('does not try again after any other refusal', async () => {
		sdk.intake.upload.mockRejectedValue(
			new IntakeUploadError('The server could not be reached', 502),
		)
		const { result } = setup()

		act(() => result.current.add([{ file: file('a.epub') }]))
		await pass(0)
		await pass(PROXY_RETRY_MAX_SECS * 1000)

		expect(statuses(result)).toEqual(['error'])
		expect(sdk.intake.upload).toHaveBeenCalledTimes(1)
	})

	it('keeps files that are added while it waits behind the book that is held back', async () => {
		sdk.intake.upload.mockRejectedValueOnce(slowDown())
		const { result } = setup()

		act(() => result.current.add([{ file: file('a.epub') }]))
		await pass(0)
		act(() => result.current.add([{ file: file('b.epub') }]))
		await pass(100)
		expect(sdk.intake.upload).toHaveBeenCalledTimes(1)

		await pass(PROXY_RETRY_MIN_SECS * 1000)
		await settle()
		expect(statuses(result)).toEqual(['done', 'done'])
		const sent = sdk.intake.upload.mock.calls.map(([call]) => call.files[0].name)
		expect(sent).toEqual(['a.epub', 'a.epub', 'b.epub'])
	})
})

describe('useIntakeInbox', () => {
	const withItems = (...states: IntakeState[]) => {
		inbox.data = { intakeItems: states.map((state, i) => item({ id: String(i), state })) }
	}
	const hidden = (value: boolean) =>
		Object.defineProperty(document, 'hidden', { configurable: true, value })

	beforeEach(() => {
		vi.clearAllMocks()
		vi.useFakeTimers()
		hidden(false)
	})
	afterEach(() => {
		vi.useRealTimers()
		hidden(false)
	})

	it('hands back the items it read', () => {
		withItems(IntakeState.Ready, IntakeState.NeedsReview)

		const { result } = renderHook(() => useIntakeInbox(), { wrapper })

		expect(result.current.items.map((i) => i.id)).toEqual(['0', '1'])
	})

	it('has no items before the first answer', () => {
		inbox.data = undefined

		const { result } = renderHook(() => useIntakeInbox(), { wrapper })

		expect(result.current.items).toEqual([])
	})

	it('reads again fast while something is being analysed', () => {
		withItems(IntakeState.Analysing)
		renderHook(() => useIntakeInbox(), { wrapper })

		vi.advanceTimersByTime(POLL_FAST_MS - 1)
		expect(inbox.refetch).not.toHaveBeenCalled()
		vi.advanceTimersByTime(1)
		expect(inbox.refetch).toHaveBeenCalledTimes(1)
		vi.advanceTimersByTime(POLL_FAST_MS)
		expect(inbox.refetch).toHaveBeenCalledTimes(2)
	})

	it('reads again fast while an approved book is being filed', () => {
		withItems(IntakeState.Filing)
		renderHook(() => useIntakeInbox(), { wrapper })

		vi.advanceTimersByTime(POLL_FAST_MS)

		expect(inbox.refetch).toHaveBeenCalledTimes(1)
	})

	it('reads again slowly while a book waits for an approver, who may take hours', () => {
		withItems(IntakeState.PendingApproval)
		renderHook(() => useIntakeInbox(), { wrapper })

		vi.advanceTimersByTime(POLL_SLOW_MS - 1)
		expect(inbox.refetch).not.toHaveBeenCalled()
		vi.advanceTimersByTime(1)
		expect(inbox.refetch).toHaveBeenCalledTimes(1)
	})

	it('does not read again when nothing moves', () => {
		withItems(IntakeState.Ready, IntakeState.Rejected)
		renderHook(() => useIntakeInbox(), { wrapper })

		vi.advanceTimersByTime(POLL_SLOW_MS * 3)

		expect(inbox.refetch).not.toHaveBeenCalled()
	})

	it('does not cancel a read that is still on its way for a new one', () => {
		withItems(IntakeState.Analysing)
		renderHook(() => useIntakeInbox(), { wrapper })

		vi.advanceTimersByTime(POLL_FAST_MS)

		expect(inbox.refetch).toHaveBeenCalledWith({ cancelRefetch: false })
	})

	it('does not read in a tab nobody is looking at', () => {
		withItems(IntakeState.Analysing)
		hidden(true)
		renderHook(() => useIntakeInbox(), { wrapper })

		vi.advanceTimersByTime(POLL_FAST_MS * 4)
		expect(inbox.refetch).not.toHaveBeenCalled()

		hidden(false)
		vi.advanceTimersByTime(POLL_FAST_MS)
		expect(inbox.refetch).toHaveBeenCalledTimes(1)
	})

	it('stops reading once the inbox is moving no more', () => {
		withItems(IntakeState.Analysing)
		const { rerender } = renderHook(() => useIntakeInbox(), { wrapper })
		vi.advanceTimersByTime(POLL_FAST_MS)
		expect(inbox.refetch).toHaveBeenCalledTimes(1)

		withItems(IntakeState.Ready)
		rerender()
		vi.advanceTimersByTime(POLL_SLOW_MS * 2)

		expect(inbox.refetch).toHaveBeenCalledTimes(1)
	})
})

describe('useIntakeActions', () => {
	// the server's tags for the book are Fantasy and Horror (the latter only a guess)
	const withTags = (state: IntakeState, genres = ['Fantasy', 'Horror']) => {
		const base = item({ state })
		if (!base.decision) throw new Error('the fixture has a decision')
		return {
			...base,
			decision: { ...base.decision, metadata: { ...base.decision.metadata, genres } },
		}
	}
	const review = () => mutationOf(intakeReviewMutation)
	const submit = () => mutationOf(intakeSubmitMutation)
	const approve = () => mutationOf(intakeApproveMutation)
	const accept = () => mutationOf(intakeAcceptMutation)
	const actions = () => renderHook(() => useIntakeActions(), { wrapper }).result

	beforeEach(() => {
		vi.clearAllMocks()
		// not what an earlier test left queued for a call that never came
		review().mutateAsync.mockReset()
		review().mutateAsync.mockResolvedValue({})
	})

	describe('a book an approver sent back', () => {
		it('is opened by a review before it is sent again, even with the tags as they were', async () => {
			const { current } = actions()
			const sentBack = withTags(IntakeState.Rejected, ['Fantasy'])

			await act(() => current.submitItem(sentBack, ['Fantasy']))

			// the server only takes a book for approval once it is ready or in need of a look
			expect(review().mutateAsync).toHaveBeenCalledTimes(1)
			expect(review().mutateAsync).toHaveBeenCalledWith({
				id: 'a',
				input: { genres: ['Fantasy'] },
			})
			expect(submit().mutate).toHaveBeenCalledWith({ ids: ['a'] })
			expect(review().mutateAsync.mock.invocationCallOrder[0]).toBeLessThan(
				submit().mutate.mock.invocationCallOrder[0] ?? 0,
			)
		})

		it('is not sent again when its review fails', async () => {
			review().mutateAsync.mockRejectedValueOnce(new Error('no'))
			const { current } = actions()

			await act(() => current.submitItem(withTags(IntakeState.Rejected, ['Fantasy']), ['Fantasy']))

			expect(submit().mutate).not.toHaveBeenCalled()
		})
	})

	describe('a book that is ready', () => {
		it('is sent as it is when the tags are the server’s', async () => {
			const { current } = actions()

			await act(() => current.submitItem(withTags(IntakeState.Ready, ['Fantasy']), ['Fantasy']))

			expect(review().mutateAsync).not.toHaveBeenCalled()
			expect(submit().mutate).toHaveBeenCalledWith({ ids: ['a'] })
		})

		it('is sent with the tags first when they are not', async () => {
			const { current } = actions()

			await act(() => current.acceptItem(withTags(IntakeState.Ready), ['Fantasy']))

			expect(review().mutateAsync).toHaveBeenCalledWith({
				id: 'a',
				input: { genres: ['Fantasy'] },
			})
			expect(accept().mutate).toHaveBeenCalledWith({ ids: ['a'] })
		})
	})

	describe('approving', () => {
		it('sends the tags the card shows first, so the book is filed with what the approver saw', async () => {
			const { current } = actions()

			await act(() => current.approveItem(withTags(IntakeState.PendingApproval), ['Fantasy']))

			expect(review().mutateAsync).toHaveBeenCalledWith({
				id: 'a',
				input: { genres: ['Fantasy'] },
			})
			expect(approve().mutate).toHaveBeenCalledWith({ ids: ['a'] })
			expect(review().mutateAsync.mock.invocationCallOrder[0]).toBeLessThan(
				approve().mutate.mock.invocationCallOrder[0] ?? 0,
			)
		})

		it('approves at once when the tags are as the server has them', async () => {
			const { current } = actions()

			await act(() =>
				current.approveItem(withTags(IntakeState.PendingApproval, ['Fantasy']), ['Fantasy']),
			)

			expect(review().mutateAsync).not.toHaveBeenCalled()
			expect(approve().mutate).toHaveBeenCalledWith({ ids: ['a'] })
		})

		it('does not approve a book whose tags could not be saved', async () => {
			review().mutateAsync.mockRejectedValueOnce(new Error('no'))
			const { current } = actions()

			await act(() => current.approveItem(withTags(IntakeState.PendingApproval), ['Fantasy']))

			expect(approve().mutate).not.toHaveBeenCalled()
		})

		it('approves a page with the tags of each book, reviewing only the books whose tags differ', async () => {
			const { current } = actions()
			const same = { ...withTags(IntakeState.PendingApproval, ['Fantasy']), id: 'b' }
			const differs = { ...withTags(IntakeState.PendingApproval), id: 'c' }

			await act(() =>
				current.approveWithGenres([
					{ genres: ['Fantasy'], item: same },
					{ genres: ['Fantasy'], item: differs },
				]),
			)

			expect(review().mutateAsync).toHaveBeenCalledTimes(1)
			expect(review().mutateAsync).toHaveBeenCalledWith({
				id: 'c',
				input: { genres: ['Fantasy'] },
			})
			// one approval, for the whole page
			expect(approve().mutate).toHaveBeenCalledTimes(1)
			expect(approve().mutate).toHaveBeenCalledWith({ ids: ['b', 'c'] })
		})
	})
})
