import {
	clampFontSize,
	LINE_SPACINGS,
	MAX_FONT_SIZE,
	MIN_FONT_SIZE,
	nearestOption,
	PAGE_MARGINS,
	shortFontName,
} from '../TextSettings'

describe('the EPUB text settings model', () => {
	it('shows a stored value on its nearest switch position', () => {
		expect(nearestOption(LINE_SPACINGS, 1.5)).toBe(1.5)
		expect(nearestOption(LINE_SPACINGS, 1.3)).toBe(1.2)
		expect(nearestOption(LINE_SPACINGS, 2.4)).toBe(1.8)
		// the old 2x margin reads as wide
		expect(nearestOption(PAGE_MARGINS, 2)).toBe(1.5)
	})

	it('keeps the font size within its bounds', () => {
		expect(clampFontSize(MIN_FONT_SIZE - 3)).toBe(MIN_FONT_SIZE)
		expect(clampFontSize(MAX_FONT_SIZE + 1)).toBe(MAX_FONT_SIZE)
		expect(clampFontSize(22.4)).toBe(22)
	})

	it('names a typeface by the word people know', () => {
		expect(shortFontName('Libre Baskerville')).toBe('Baskerville')
		expect(shortFontName('Hina Mincho')).toBe('Mincho')
		expect(shortFontName('Atkinson Hyperlegible Next')).toBe('Atkinson')
		expect(shortFontName('Literata')).toBe('Literata')
	})
})
