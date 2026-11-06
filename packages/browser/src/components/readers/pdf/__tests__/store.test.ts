import {
	PDF_DEFAULT_KEYS,
	PDF_RESET_KEYS,
	pdfEffectiveDefaults,
	pdfHasOwnSettings,
	promotePdfDefaults,
	usePdfReaderStore,
} from '../store'

describe("every PDF's defaults", () => {
	beforeEach(() => usePdfReaderStore.setState({ books: {}, defaults: {} }))

	it("makes a PDF's fit, layout and look every PDF's, not its zoom or how it opens", () => {
		const state = {
			defaults: { invertPagesInDark: false },
			books: {
				a: { fit: 'width' as const, spread: 'double' as const, zoom: 1.5, reader: 'pdf' as const },
				b: { fit: 'page' as const },
			},
		}
		const next = promotePdfDefaults(state, 'a')
		expect(next.defaults).toEqual({ invertPagesInDark: false, fit: 'width', spread: 'double' })
		expect(next.books.a).toEqual({ zoom: 1.5, reader: 'pdf' })
		// other books keep their own settings
		expect(next.books.b).toEqual({ fit: 'page' })
		expect(PDF_DEFAULT_KEYS).not.toContain('zoom')
		expect(PDF_RESET_KEYS).toContain('zoom')
	})

	it('reads a book over the defaults and resets it to them', () => {
		const store = usePdfReaderStore.getState()
		store.setBookPreferences('a', { fit: 'width', scrollMode: 'paged' })
		usePdfReaderStore.getState().makeDefault('a')
		expect(usePdfReaderStore.getState().defaults).toEqual({ fit: 'width', scrollMode: 'paged' })
		expect(usePdfReaderStore.getState().books.a).toBeUndefined()

		usePdfReaderStore.getState().setBookPreferences('b', { fit: 'page', zoom: 2, reader: 'pages' })
		usePdfReaderStore.getState().resetBook('b')
		// how it opens isn't a setting to reset
		expect(usePdfReaderStore.getState().books.b).toEqual({ reader: 'pages' })
	})

	it('tells a book with its own settings from one that follows the defaults', () => {
		// built in: fit width on a phone, fit page on a larger screen
		expect(pdfEffectiveDefaults({}, true).fit).toBe('width')
		expect(pdfEffectiveDefaults({}, false).fit).toBe('page')
		expect(pdfHasOwnSettings(undefined, {}, false)).toBe(false)
		expect(pdfHasOwnSettings({ fit: 'page', zoom: 1 }, {}, false)).toBe(false)
		expect(pdfHasOwnSettings({ fit: 'page' }, {}, true)).toBe(true)
		expect(pdfHasOwnSettings({ fit: 'width' }, { fit: 'width' }, false)).toBe(false)
		expect(pdfHasOwnSettings({ zoom: 1.5 }, {}, false)).toBe(true)
		expect(pdfHasOwnSettings({ reader: 'pages' }, {}, false)).toBe(false)
		// margins trimmed on a phone, not on a larger screen
		expect(pdfEffectiveDefaults({}, true).trimMargins).toBe(true)
		expect(pdfEffectiveDefaults({}, false).trimMargins).toBe(false)
		expect(pdfHasOwnSettings({ trimMargins: true }, {}, false)).toBe(true)
		expect(pdfHasOwnSettings({ trimMargins: true }, {}, true)).toBe(false)
		expect(PDF_DEFAULT_KEYS).toContain('trimMargins')
	})
})
