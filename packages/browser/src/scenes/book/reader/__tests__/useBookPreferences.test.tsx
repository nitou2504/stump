import { ReadingDirection, ReadingImageScaleFit } from '@stump/graphql'
import { act, renderHook } from '@testing-library/react'

import type { ImageReaderBookRef } from '@/components/readers/imageBased/context'
import { useReaderStore } from '@/stores'

import { useBookPreferences } from '../useBookPreferences'

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

	it('crops margins and fits the width by default on a phone, not on a desktop', () => {
		setScreen(390, 844)
		const phone = renderHook(() => useBookPreferences({ book: manga })).result.current
		expect(phone.bookPreferences.cropBorders).toBe(true)
		expect(phone.bookPreferences.imageScaling.scaleToFit).toBe(ReadingImageScaleFit.Width)
		expect(phone.bookPreferences.readingDirection).toBe(ReadingDirection.Rtl)

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
})
