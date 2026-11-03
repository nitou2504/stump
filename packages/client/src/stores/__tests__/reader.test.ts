import { describe, expect, it } from 'vitest'

import {
	BOOK_RESET_KEYS,
	createReaderStore,
	DEFAULT_EPUB_FONT_SIZE,
	FORMAT_DEFAULT_KEYS,
	formatKindOf,
	migrateReaderStore,
	promoteToFormatDefaults,
	type ReaderStore,
	resetBookToDefaults,
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

describe('format defaults (version 10)', () => {
	it('keeps a version 9 store as it was: no defaults until "Make default"', () => {
		const book = { fontSize: 26 }
		const migrated = migrateReaderStore(
			{ settings: { fontSize: 22 }, bookPreferences: { book } } as unknown as ReaderStore,
			9,
		)
		expect(migrated.bookPreferences.book).toEqual(book)
		expect(migrated.formatDefaults).toBeUndefined()
		// a malformed value is replaced
		const broken = migrateReaderStore(
			{ formatDefaults: 'oops', bookPreferences: {} } as unknown as ReaderStore,
			9,
		)
		expect(broken.formatDefaults).toEqual({})
	})

	it('tells an EPUB from the books the image reader opens', () => {
		expect(formatKindOf({ extension: 'epub' })).toBe('epub')
		expect(formatKindOf({ extension: 'EPUB' })).toBe('epub')
		expect(formatKindOf({ extension: 'cbz' })).toBe('comic')
		expect(formatKindOf({ extension: 'pdf' })).toBe('comic')
		expect(formatKindOf({})).toBe('comic')
	})

	it("makes a book's settings the defaults of its kind and leaves its reading direction", () => {
		const state = {
			formatDefaults: { epub: { lineHeight: 1.8 } },
			bookPreferences: {
				a: { fontSize: 26, fontFamily: 'LITERATA', readingDirection: 'RTL' },
				b: { fontSize: 18 },
			},
		} as unknown as Pick<ReaderStore, 'formatDefaults' | 'bookPreferences'>
		const next = promoteToFormatDefaults(state, 'epub', 'a')
		expect(next.formatDefaults.epub).toEqual({
			lineHeight: 1.8,
			fontSize: 26,
			fontFamily: 'LITERATA',
		})
		// the book reads them from the defaults now; its direction is its own
		expect(next.bookPreferences.a).toEqual({ readingDirection: 'RTL' })
		// other books keep their own settings
		expect(next.bookPreferences.b).toEqual({ fontSize: 18 })
		// comics keep theirs
		expect(next.formatDefaults.comic).toBeUndefined()
	})

	it('drops a book left without settings, and ignores a book without any', () => {
		const state = {
			formatDefaults: {},
			bookPreferences: { a: { imageScaling: { scaleToFit: 'WIDTH' } } },
		} as unknown as Pick<ReaderStore, 'formatDefaults' | 'bookPreferences'>
		const next = promoteToFormatDefaults(state, 'comic', 'a')
		expect(next.formatDefaults.comic).toEqual({ imageScaling: { scaleToFit: 'WIDTH' } })
		expect(next.bookPreferences).toEqual({})
		expect(promoteToFormatDefaults(state, 'comic', 'nothing')).toBe(state)
	})

	it("resets a book to its kind's defaults, reading direction included", () => {
		const books = {
			a: { fontSize: 26, readingDirection: 'RTL', brightness: 0.5 },
			b: { fontSize: 18 },
		} as unknown as ReaderStore['bookPreferences']
		// an EPUB keeps what isn't an EPUB setting (a comic's brightness)
		expect(resetBookToDefaults(books, 'epub', 'a')).toEqual({
			a: { brightness: 0.5 },
			b: { fontSize: 18 },
		})
		expect(resetBookToDefaults(books, 'epub', 'b')).toEqual({ a: books.a })
		expect(BOOK_RESET_KEYS.comic).toEqual([...FORMAT_DEFAULT_KEYS.comic, 'readingDirection'])
	})

	it('does both from the store', () => {
		const memory = new Map<string, string>()
		const store = createReaderStore({
			getItem: (key) => memory.get(key) ?? null,
			setItem: (key, value) => void memory.set(key, value),
			removeItem: (key) => void memory.delete(key),
		})
		store.getState().setBookPreferences('a', { fontSize: 30 } as never)
		store.getState().setBookPreferences('b', { fontSize: 18 } as never)
		store.getState().makeFormatDefault('epub', 'a')
		expect(store.getState().formatDefaults.epub).toEqual({ fontSize: 30 })
		expect(store.getState().bookPreferences.a).toBeUndefined()
		store.getState().resetBookPreferences('epub', 'b')
		expect(store.getState().bookPreferences).toEqual({})
		// kept in the browser with the rest of the reader settings
		expect(JSON.parse(memory.get('stump-new-reader-store')!)).toMatchObject({
			version: 10,
			state: { formatDefaults: { epub: { fontSize: 30 } } },
		})
	})
})
