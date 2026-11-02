import { describe, expect, it } from 'vitest'

import { DEFAULT_EPUB_FONT_SIZE, migrateReaderStore, type ReaderStore } from '../reader'

describe('migrateReaderStore', () => {
	it('raises the old default font size (13px) to the new default, in settings and books', () => {
		const migrated = migrateReaderStore(
			{
				settings: { fontSize: 13, lineHeight: 1.5 },
				bookPreferences: {
					'never-changed-size': { fontSize: 13, lineHeight: 1.8 },
					'picked-size': { fontSize: 18 },
				},
				bookTimers: { 'some-book': 42 },
			},
			5,
		)

		expect(migrated.settings.fontSize).toBe(DEFAULT_EPUB_FONT_SIZE)
		expect(migrated.settings.lineHeight).toBe(1.5)
		expect(migrated.bookPreferences['never-changed-size']).toEqual({
			fontSize: DEFAULT_EPUB_FONT_SIZE,
			lineHeight: 1.8,
		})
		expect(migrated.bookPreferences['picked-size']?.fontSize).toBe(18)
		expect(migrated.bookTimers).toEqual({ 'some-book': 42 })
	})

	it('keeps a font size the user picked', () => {
		const migrated = migrateReaderStore({ settings: { fontSize: 16 }, bookPreferences: {} }, 5)
		expect(migrated.settings.fontSize).toBe(16)
	})

	it('drops stores older than version 5, like before migrations existed', () => {
		expect(migrateReaderStore({ settings: { fontSize: 13 } }, 4)).toBeUndefined()
	})

	it('copes with a store without settings or books', () => {
		expect(migrateReaderStore({}, 5)).toEqual({} as ReaderStore)
	})

	it('keeps a page number that was turned off off in every reader (version 7)', () => {
		const migrated = migrateReaderStore(
			{ settings: { fontSize: 22, showPageIndicator: false }, bookPreferences: {} },
			6,
		)
		expect(migrated.settings.readingProgress).toEqual({
			epub: { display: 'off' },
			pdf: { display: 'off' },
			comic: { display: 'off' },
		})
	})

	it("leaves each reader's progress to its defaults when the page number was shown", () => {
		const migrated = migrateReaderStore(
			{ settings: { fontSize: 22, showPageIndicator: true }, bookPreferences: {} },
			6,
		)
		expect(migrated.settings.readingProgress).toBeUndefined()
		const untouched = migrateReaderStore({ settings: { fontSize: 22 }, bookPreferences: {} }, 6)
		expect(untouched.settings.readingProgress).toBeUndefined()
	})
})
