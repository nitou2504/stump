import { ReadingImageScaleFit } from '@stump/graphql'

import { fitForScreen } from '../useBookPreferences'

describe('fitForScreen', () => {
	it('shows the whole page instead of fitting the height on a portrait phone', () => {
		expect(fitForScreen(ReadingImageScaleFit.Height, { width: 390, height: 844 })).toBe(
			ReadingImageScaleFit.Auto,
		)
	})

	it('keeps fit-to-height on wide or landscape screens', () => {
		expect(fitForScreen(ReadingImageScaleFit.Height, { width: 1440, height: 900 })).toBe(
			ReadingImageScaleFit.Height,
		)
		expect(fitForScreen(ReadingImageScaleFit.Height, { width: 600, height: 400 })).toBe(
			ReadingImageScaleFit.Height,
		)
	})

	it('leaves the other fits alone', () => {
		expect(fitForScreen(ReadingImageScaleFit.Width, { width: 390, height: 844 })).toBe(
			ReadingImageScaleFit.Width,
		)
		expect(fitForScreen(ReadingImageScaleFit.None, { width: 390, height: 844 })).toBe(
			ReadingImageScaleFit.None,
		)
	})
})
