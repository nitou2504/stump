import { ReadingDirection, ReadingImageScaleFit, ReadingMode } from '@stump/graphql'
import { act, renderHook } from '@testing-library/react'

import type { ImageReaderBookRef } from '@/components/readers/imageBased/context'
import { useReaderStore } from '@/stores'

import { hasOwnSettings, useBookPreferences } from '../useBookPreferences'

const manga = {
	id: 'manga-1',
	metadata: { genres: ['Manga & Comics: Manga'] },
	libraryConfig: { defaultReadingImageScaleFit: ReadingImageScaleFit.Height },
} as unknown as ImageReaderBookRef

function setScreen(width: number, height: number) {
	Object.defineProperty(window, 'innerWidth', { configurable: true, value: width })
	Object.defineProperty(window, 'innerHeight', { configurable: true, value: height })
}

describe('useBookPreferences: per-device defaults', () => {
	beforeEach(() => {
		act(() => useReaderStore.setState({ bookPreferences: {} }))
	})

	it('crops margins and fits the page (upright) or the width (on its side) on a phone', () => {
		setScreen(390, 844)
		const phone = renderHook(() => useBookPreferences({ book: manga })).result.current
		expect(phone.bookPreferences.cropBorders).toBe(true)
		expect(phone.bookPreferences.imageScaling.scaleToFit).toBe(ReadingImageScaleFit.Auto)
		expect(phone.bookPreferences.readingDirection).toBe(ReadingDirection.Rtl)

		setScreen(844, 390)
		const sideways = renderHook(() => useBookPreferences({ book: manga })).result.current
		expect(sideways.bookPreferences.imageScaling.scaleToFit).toBe(ReadingImageScaleFit.Width)

		setScreen(1440, 900)
		const desktop = renderHook(() => useBookPreferences({ book: manga })).result.current
		expect(desktop.bookPreferences.cropBorders).toBe(false)
		expect(desktop.bookPreferences.imageScaling.scaleToFit).toBe(ReadingImageScaleFit.Height)
	})

	it("keeps only what was changed, so a phone's defaults don't follow the book to a desktop", () => {
		setScreen(390, 844)
		const { result } = renderHook(() => useBookPreferences({ book: manga }))
		act(() => result.current.setBookPreferences({ widePages: 'split' }))
		expect(useReaderStore.getState().bookPreferences[manga.id]).toEqual({ widePages: 'split' })

		setScreen(1440, 900)
		const desktop = renderHook(() => useBookPreferences({ book: manga })).result.current
		expect(desktop.bookPreferences.widePages).toBe('split')
		// the phone's fit-width and crop weren't stored with it
		expect(desktop.bookPreferences.imageScaling.scaleToFit).toBe(ReadingImageScaleFit.Height)
		expect(desktop.bookPreferences.cropBorders).toBe(false)
	})

	it('follows the phone when it turns', () => {
		setScreen(390, 844)
		const { result } = renderHook(() => useBookPreferences({ book: manga }))
		expect(result.current.bookPreferences.imageScaling.scaleToFit).toBe(ReadingImageScaleFit.Auto)
		act(() => {
			setScreen(844, 390)
			window.dispatchEvent(new Event('resize'))
		})
		expect(result.current.bookPreferences.imageScaling.scaleToFit).toBe(ReadingImageScaleFit.Width)
	})

	it('keeps a fit chosen for the book on every screen', () => {
		setScreen(390, 844)
		const { result } = renderHook(() => useBookPreferences({ book: manga }))
		act(() =>
			result.current.setBookPreferences({
				imageScaling: { scaleToFit: ReadingImageScaleFit.Height },
			}),
		)
		expect(result.current.bookPreferences.imageScaling.scaleToFit).toBe(ReadingImageScaleFit.Height)
	})
})

const epubA = {
	id: 'epub-a',
	extension: 'epub',
	libraryConfig: { defaultReadingMode: ReadingMode.Paged },
} as unknown as ImageReaderBookRef
const epubB = { ...epubA, id: 'epub-b' } as ImageReaderBookRef
const comic = { ...manga, id: 'comic-1', extension: 'cbz', metadata: null } as ImageReaderBookRef

describe('useBookPreferences: defaults for every EPUB or comic', () => {
	beforeEach(() => {
		setScreen(1440, 900)
		act(() => useReaderStore.setState({ bookPreferences: {}, formatDefaults: {} }))
	})

	it("makes one book's settings every EPUB's, and resets a book to them", () => {
		const a = renderHook(() => useBookPreferences({ book: epubA }))
		expect(a.result.current.kind).toBe('epub')
		expect(a.result.current.differsFromDefaults).toBe(false)

		act(() => a.result.current.setBookPreferences({ fontSize: 28, fontFamily: 'LITERATA' }))
		expect(a.result.current.differsFromDefaults).toBe(true)
		act(() => a.result.current.makeDefault())
		// the book now follows the defaults, which are its settings
		expect(a.result.current.differsFromDefaults).toBe(false)
		expect(a.result.current.bookPreferences.fontSize).toBe(28)
		expect(useReaderStore.getState().bookPreferences[epubA.id]).toBeUndefined()

		// another EPUB opens with them
		const b = renderHook(() => useBookPreferences({ book: epubB }))
		expect(b.result.current.bookPreferences.fontSize).toBe(28)
		expect(b.result.current.bookPreferences.fontFamily).toBe('LITERATA')
		// and a comic doesn't
		const c = renderHook(() => useBookPreferences({ book: comic }))
		expect(c.result.current.kind).toBe('comic')
		expect(c.result.current.bookPreferences.fontFamily).toBeUndefined()

		// a book's own choice wins, the book's font included, and a reset brings the defaults back
		act(() => b.result.current.setBookPreferences({ fontSize: 18, fontFamily: '' }))
		expect(b.result.current.bookPreferences.fontFamily).toBe('')
		expect(b.result.current.differsFromDefaults).toBe(true)
		act(() => b.result.current.resetToDefaults())
		expect(b.result.current.bookPreferences.fontSize).toBe(28)
		expect(b.result.current.differsFromDefaults).toBe(false)
	})

	it("puts a comic's defaults over the library's", () => {
		const c = renderHook(() => useBookPreferences({ book: comic }))
		expect(c.result.current.bookPreferences.imageScaling.scaleToFit).toBe(
			ReadingImageScaleFit.Height,
		)
		act(() =>
			c.result.current.setBookPreferences({
				imageScaling: { scaleToFit: ReadingImageScaleFit.Width },
			}),
		)
		act(() => c.result.current.makeDefault())
		const other = renderHook(() =>
			useBookPreferences({ book: { ...comic, id: 'comic-2' } as ImageReaderBookRef }),
		)
		expect(other.result.current.bookPreferences.imageScaling.scaleToFit).toBe(
			ReadingImageScaleFit.Width,
		)
	})
})

describe('hasOwnSettings', () => {
	const defaults = {
		fontSize: 22,
		columnCount: 'auto' as const,
		readingDirection: ReadingDirection.Ltr,
	}

	it('counts only settings that differ from the defaults', () => {
		expect(hasOwnSettings('epub', undefined, defaults)).toBe(false)
		expect(hasOwnSettings('epub', { fontSize: 22 }, defaults)).toBe(false)
		expect(hasOwnSettings('epub', { fontSize: 24 }, defaults)).toBe(true)
		// one page is the old 'auto'
		expect(hasOwnSettings('epub', { columnCount: 1 }, defaults)).toBe(false)
		expect(hasOwnSettings('epub', { columnCount: 2 }, defaults)).toBe(true)
		// the reading direction counts: a reset takes it back too
		expect(hasOwnSettings('epub', { readingDirection: ReadingDirection.Rtl }, defaults)).toBe(true)
		// a comic's settings aren't an EPUB's
		expect(hasOwnSettings('epub', { brightness: 0.4 }, defaults)).toBe(false)
	})
})
