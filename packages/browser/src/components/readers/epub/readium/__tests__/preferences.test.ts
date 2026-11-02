import { ReadingMode } from '@stump/graphql'

import { bookPreferencesToEpubPreferences, resolveEffectiveColumnCount } from '../preferences'

describe('resolveEffectiveColumnCount', () => {
	it('shows two columns (a two-page spread) only when picked and the stage is wide enough', () => {
		expect(resolveEffectiveColumnCount(2, 1400)).toBe(2)
		expect(resolveEffectiveColumnCount(2, 900)).toBe(2)
		expect(resolveEffectiveColumnCount(2, 899)).toBe(1)
		expect(resolveEffectiveColumnCount(2, undefined)).toBe(1)
	})

	it('shows one column otherwise, including the old auto default', () => {
		expect(resolveEffectiveColumnCount(1, 2000)).toBe(1)
		expect(resolveEffectiveColumnCount('auto', 1400)).toBe(1)
		expect(resolveEffectiveColumnCount(undefined, 1400)).toBe(1)
	})
})

describe('bookPreferencesToEpubPreferences', () => {
	it('clears columnCount with null (not undefined) in continuous scroll mode', () => {
		const preferences = bookPreferencesToEpubPreferences({
			readingMode: ReadingMode.ContinuousVertical,
			columnCount: 2,
			stageWidth: 1400,
		})

		expect(preferences.scroll).toBe(true)
		expect(preferences.columnCount).toBeNull()
		expect(preferences.columnCount).not.toBeUndefined()
	})

	it('resolves an explicit column count in paginated mode', () => {
		const preferences = bookPreferencesToEpubPreferences({
			readingMode: ReadingMode.Paged,
			columnCount: 1,
			stageWidth: 1400,
		})

		expect(preferences.scroll).toBe(false)
		expect(preferences.columnCount).toBe(1)
	})

	it('shows two columns in paginated mode only when picked and the stage is wide', () => {
		const wide = bookPreferencesToEpubPreferences({
			readingMode: ReadingMode.Paged,
			columnCount: 2,
			stageWidth: 1400,
		})
		const narrow = bookPreferencesToEpubPreferences({
			readingMode: ReadingMode.Paged,
			columnCount: 2,
			stageWidth: 400,
		})
		const auto = bookPreferencesToEpubPreferences({
			readingMode: ReadingMode.Paged,
			columnCount: 'auto',
			stageWidth: 1400,
		})

		expect(wide.columnCount).toBe(2)
		expect(narrow.columnCount).toBe(1)
		expect(auto.columnCount).toBe(1)
	})

	it('maps pageMargins to pageGutter only in paginated mode', () => {
		const paginated = bookPreferencesToEpubPreferences({
			readingMode: ReadingMode.Paged,
			pageMargins: 1.5,
		})
		const continuous = bookPreferencesToEpubPreferences({
			readingMode: ReadingMode.ContinuousVertical,
			pageMargins: 1.5,
		})

		expect(paginated.pageGutter).toBeGreaterThan(0)
		expect(continuous.pageGutter).toBeUndefined()
	})

	it('maps fontSize (px) to a Readium scale factor', () => {
		const preferences = bookPreferencesToEpubPreferences({ fontSize: 16 })
		expect(preferences.fontSize).toBeCloseTo(1, 5)
	})

	it('applies a dark palette when isDarkVariant is true', () => {
		const preferences = bookPreferencesToEpubPreferences({ isDarkVariant: true })
		expect(preferences.backgroundColor).toBe('#161719')
		expect(preferences.textColor).toBe('#E8EDF4')
	})
})

describe('reader themes and alignment', () => {
	it('uses an explicit theme regardless of the app theme', () => {
		const sepia = bookPreferencesToEpubPreferences({ theme: 'sepia', isDarkVariant: true })
		expect(sepia.backgroundColor).toBe('#f4ecd8')
		const black = bookPreferencesToEpubPreferences({ theme: 'black', isDarkVariant: false })
		expect(black.backgroundColor).toBe('#000000')
	})

	it('follows the app theme on auto', () => {
		expect(
			bookPreferencesToEpubPreferences({ theme: 'auto', isDarkVariant: true }).backgroundColor,
		).toBe('#161719')
		expect(
			bookPreferencesToEpubPreferences({ theme: 'auto', isDarkVariant: false }).backgroundColor,
		).toBe('#ffffff')
	})

	it('justifies with hyphenation, and clears alignment with null on auto', () => {
		const justified = bookPreferencesToEpubPreferences({ textAlign: 'justify' })
		expect(justified.textAlign).toBe('justify')
		expect(justified.hyphens).toBe(true)
		const auto = bookPreferencesToEpubPreferences({ textAlign: 'auto' })
		expect(auto.textAlign).toBeNull()
		expect(auto.hyphens).toBeNull()
	})
})
