import { ReadingImageScaleFit } from '@stump/graphql'

import { fitForScreen, isPhoneScreen } from '../useBookPreferences'

describe('fitForScreen', () => {
	it('fits the width instead of the height on a phone, in portrait and in landscape', () => {
		expect(fitForScreen(ReadingImageScaleFit.Height, { width: 390, height: 844 })).toBe(
			ReadingImageScaleFit.Width,
		)
		expect(fitForScreen(ReadingImageScaleFit.Height, { width: 844, height: 390 })).toBe(
			ReadingImageScaleFit.Width,
		)
	})

	it('keeps fit-to-height on desktops and large tablets', () => {
		expect(fitForScreen(ReadingImageScaleFit.Height, { width: 1440, height: 900 })).toBe(
			ReadingImageScaleFit.Height,
		)
		expect(fitForScreen(ReadingImageScaleFit.Height, { width: 1024, height: 768 })).toBe(
			ReadingImageScaleFit.Height,
		)
	})

	it('leaves the other fits alone', () => {
		expect(fitForScreen(ReadingImageScaleFit.Auto, { width: 390, height: 844 })).toBe(
			ReadingImageScaleFit.Auto,
		)
		expect(fitForScreen(ReadingImageScaleFit.None, { width: 390, height: 844 })).toBe(
			ReadingImageScaleFit.None,
		)
	})
})

describe('isPhoneScreen', () => {
	it('tells phones (either way round) from wider screens', () => {
		expect(isPhoneScreen({ width: 360, height: 800 })).toBe(true)
		expect(isPhoneScreen({ width: 915, height: 412 })).toBe(true)
		expect(isPhoneScreen({ width: 820, height: 1180 })).toBe(true)
		expect(isPhoneScreen({ width: 1180, height: 820 })).toBe(false)
		expect(isPhoneScreen({ width: 1920, height: 1080 })).toBe(false)
	})
})
