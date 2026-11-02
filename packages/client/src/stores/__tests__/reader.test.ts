import { describe, expect, it } from 'vitest'

import {
	DEFAULT_EPUB_FONT_SIZE,
	migrateReaderStore,
	type ReaderStore,
	withoutFrozenDefaults,
} from '../reader'

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

	it("drops the reading bar's 'show' and keeps each reader's display and items (version 8)", () => {
		const migrated = migrateReaderStore(
			{
				settings: {
					fontSize: 22,
					readingProgress: {
						epub: { display: 'bar', items: ['chapter', 'percent'], show: 'withControls' },
						pdf: { display: 'bar', show: 'always' },
						comic: { display: 'number' },
					},
				},
				bookPreferences: {},
			} as unknown as ReaderStore,
			7,
		)
		expect(migrated.settings.readingProgress).toEqual({
			epub: { display: 'bar', items: ['chapter', 'percent'] },
			pdf: { display: 'bar' },
			comic: { display: 'number' },
		})
	})

	it("drops the comic's page background, in the settings and every book (version 8)", () => {
		const migrated = migrateReaderStore(
			{
				settings: { fontSize: 22, pageBackground: 'black', brightness: 0.8 },
				bookPreferences: {
					manga: { pageBackground: 'auto', cropBorders: true },
					novel: { fontSize: 20 },
				},
			} as unknown as ReaderStore,
			7,
		)
		expect(migrated.settings).toEqual({ fontSize: 22, brightness: 0.8 })
		expect(migrated.bookPreferences).toEqual({
			manga: { cropBorders: true },
			novel: { fontSize: 20 },
		})
	})

	it("thaws the defaults older builds froze into a book's preferences (version 9)", () => {
		// what any change saved before: every preference in effect then (a manga opened
		// before manga read right to left by default, its brightness changed)
		const frozen = {
			animatedReader: false,
			brightness: 0.8,
			imageScaling: { scaleToFit: 'HEIGHT' },
			readingDirection: 'LTR',
			readingMode: 'PAGED',
			tapSidesToNavigate: true,
			fontSize: 13,
			lineHeight: 1.5,
			trackElapsedTime: true,
			doublePageBehavior: 'off',
			secondPageSeparate: false,
			panzoomWithoutCtrl: false,
			columnCount: 'auto',
			pageMargins: 1,
		}
		const migrated = migrateReaderStore(
			{
				settings: { fontSize: 22 },
				bookPreferences: {
					manga: frozen,
					// a later change on top of a copy: a choice that isn't the default stays
					picked: { ...frozen, readingDirection: 'RTL', imageScaling: { scaleToFit: 'WIDTH' } },
					// only what was changed (version 8 onwards): left alone
					chosen: { readingDirection: 'LTR', widePages: 'split' },
				},
			} as unknown as ReaderStore,
			8,
		)
		expect(migrated.bookPreferences.manga).toEqual({ brightness: 0.8 })
		expect(migrated.bookPreferences.picked).toEqual({
			brightness: 0.8,
			readingDirection: 'RTL',
			imageScaling: { scaleToFit: 'WIDTH' },
		})
		expect(migrated.bookPreferences.chosen).toEqual({ readingDirection: 'LTR', widePages: 'split' })
	})

	it('leaves version 9 stores and changes-only preferences alone', () => {
		const book = { readingDirection: 'LTR', animatedReader: false }
		const migrated = migrateReaderStore(
			{ settings: { fontSize: 22 }, bookPreferences: { book } } as unknown as ReaderStore,
			9,
		)
		expect(migrated.bookPreferences.book).toEqual(book)
		expect(withoutFrozenDefaults({ cropBorders: true })).toEqual({ cropBorders: true })
	})
})
