import { ReadingImageScaleFit } from '@stump/graphql'

import { fitForScreen, isPhoneScreen } from '../useBookPreferences'

describe('fitForScreen', () => {
	it('fits the page on an upright phone and the width on a phone on its side', () => {
		// upright, a manga page then fills the width, and wide pages get Fit Page's handling
		expect(fitForScreen(ReadingImageScaleFit.Height, { width: 390, height: 844 })).toBe(
			ReadingImageScaleFit.Auto,
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

	it('does the same for a fit-page library default', () => {
		expect(fitForScreen(ReadingImageScaleFit.Auto, { width: 844, height: 390 })).toBe(
			ReadingImageScaleFit.Width,
		)
		expect(fitForScreen(ReadingImageScaleFit.Auto, { width: 390, height: 844 })).toBe(
			ReadingImageScaleFit.Auto,
		)
		expect(fitForScreen(ReadingImageScaleFit.Auto, { width: 1440, height: 900 })).toBe(
			ReadingImageScaleFit.Auto,
		)
	})

	it('leaves the original size alone', () => {
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
