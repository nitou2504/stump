import { useSDK } from '@stump/client'
import { act, renderHook } from '@testing-library/react'

import { usePdfReaderStore } from '@/components/readers/pdf/store'

import { useReaderStore } from '../reader'
import {
	parseSnapshot,
	READER_SETTINGS_VERSION,
	snapshotOf,
	useReaderSettingsSync,
} from '../readerSettingsSync'

vi.mock('@stump/client', async (importOriginal) => ({
	...(await importOriginal<typeof import('@stump/client')>()),
	useSDK: vi.fn(),
}))

/** A server holding one user's saved text */
function fakeServer(saved: string | null) {
	const server = { saved, sends: [] as string[] }
	const execute = vi.fn(async (document: unknown, variables?: { settings: string }) => {
		if (String(document).includes('mutation')) {
			server.saved = variables!.settings
			server.sends.push(variables!.settings)
			return { updateMyReaderSettings: true }
		}
		return { myReaderSettings: server.saved }
	})
	vi.mocked(useSDK).mockReturnValue({ sdk: { execute } } as never)
	return server
}

const flush = () => act(async () => {})

describe('reading settings on the server', () => {
	beforeEach(() => {
		vi.useFakeTimers()
		useReaderStore.setState((state) => ({
			settings: { ...state.settings, theme: 'black', showToolBar: false },
			formatDefaults: {},
			bookPreferences: {},
		}))
		usePdfReaderStore.setState({ books: {}, defaults: {} })
	})
	afterEach(() => vi.useRealTimers())

	it('keeps the toolbar out and reads back only its own shape', () => {
		const snapshot = snapshotOf(useReaderStore.getState(), usePdfReaderStore.getState())
		expect(snapshot.v).toBe(READER_SETTINGS_VERSION)
		expect(snapshot.reader.settings).not.toHaveProperty('showToolBar')
		expect(parseSnapshot(JSON.stringify(snapshot))).toEqual(snapshot)
		expect(parseSnapshot('{"v":0}')).toBeNull()
		expect(parseSnapshot('nope')).toBeNull()
		expect(parseSnapshot(null)).toBeNull()
	})

	it("seeds an empty server with the browser's settings", async () => {
		const server = fakeServer(null)
		renderHook(() => useReaderSettingsSync('user'))
		await flush()
		expect(server.sends).toHaveLength(1)
		expect(parseSnapshot(server.saved)!.reader.settings.theme).toBe('black')
	})

	it("replaces the browser's settings with the server's, then sends changes", async () => {
		const saved = snapshotOf(useReaderStore.getState(), usePdfReaderStore.getState())
		saved.reader.settings.theme = 'sepia'
		saved.pdf.defaults = { spread: 'double', scrollMode: 'paged' }
		const server = fakeServer(JSON.stringify(saved))
		renderHook(() => useReaderSettingsSync('user'))
		await flush()
		expect(useReaderStore.getState().settings.theme).toBe('sepia')
		expect(usePdfReaderStore.getState().defaults).toEqual({ spread: 'double', scrollMode: 'paged' })
		// applying the server's copy doesn't send it back
		expect(server.sends).toHaveLength(0)

		act(() => usePdfReaderStore.getState().setBookPreferences('book', { fit: 'width' }))
		// the toolbar alone isn't worth a save
		act(() => useReaderStore.getState().setSettings({ showToolBar: true }))
		expect(server.sends).toHaveLength(0)
		await act(async () => vi.advanceTimersByTime(1000))
		expect(server.sends).toHaveLength(1)
		expect(parseSnapshot(server.saved)!.pdf.books).toEqual({ book: { fit: 'width' } })
	})

	it('catches up with another device when the tab comes back', async () => {
		const server = fakeServer(null)
		renderHook(() => useReaderSettingsSync('user'))
		await flush()
		const other = parseSnapshot(server.saved)!
		other.reader.settings.theme = 'light'
		server.saved = JSON.stringify(other)

		Object.defineProperty(document, 'visibilityState', { value: 'visible', configurable: true })
		await act(async () => {
			document.dispatchEvent(new Event('visibilitychange'))
		})
		await flush()
		expect(useReaderStore.getState().settings.theme).toBe('light')
	})
})
