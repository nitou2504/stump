import { IntakeState, IntakeUploadMode } from '@stump/graphql'
import { IntakeUploadError } from '@stump/sdk'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act, renderHook, waitFor } from '@testing-library/react'
import type { PropsWithChildren } from 'react'

import { useIntakeInbox, useIntakeUploader } from '../hooks'
import type { IntakeLimitsData } from '../queries'
import { POLL_FAST_MS, POLL_SLOW_MS } from '../reviewState'
import { item } from './fixtures'

const sdk = vi.hoisted(() => ({ auth: { me: vi.fn() }, intake: { upload: vi.fn() } }))
const inbox = vi.hoisted(() => ({ data: undefined as unknown, refetch: vi.fn() }))
vi.mock('@stump/client', async (importOriginal) => ({
	...(await importOriginal<typeof import('@stump/client')>()),
	useGraphQL: () => ({ data: inbox.data, refetch: inbox.refetch }),
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

	it('forgets the finished uploads and keeps the rest', async () => {
		const { result } = setup(limits({ maxFileBytes: 50 * MB }))
		act(() => result.current.add([{ file: file('huge.cbz', 80) }, { file: file('ok.epub') }]))
		await waitFor(() => expect(result.current.entries.some((e) => e.status === 'done')).toBe(true))

		act(() => result.current.clearFinished())

		expect(result.current.entries.map((e) => e.status)).toEqual(['error'])
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
