import {
	compensatedMediaCap,
	IMAGE_PAGE_ATTRIBUTE,
	isImageOnlyPage,
	syncFrameMedia,
} from '../fitFrameMedia'

/** A frame document as Readium leaves it: ReadiumCSS's media cap on `<html>`, a given body */
function frameDocument(bodyHtml: string, { cap = '95vh', style = '' } = {}) {
	// an iframe's document has a window, like the reader's frames
	const iframe = document.createElement('iframe')
	document.body.appendChild(iframe)
	const doc = iframe.contentDocument!
	if (cap) doc.documentElement.style.setProperty('--RS__maxMediaHeight', cap)
	if (style) {
		const current = doc.documentElement.getAttribute('style') ?? ''
		doc.documentElement.setAttribute('style', `${current} ${style}`.trim())
	}
	doc.body.innerHTML = bodyHtml
	return doc
}

afterEach(() => {
	document.body.innerHTML = ''
})

/** jsdom lays nothing out: how wide the page's content runs, against the page width */
function setColumns(doc: Document, contentWidth: number, pageWidth: number) {
	Object.defineProperty(doc.documentElement, 'scrollWidth', {
		configurable: true,
		value: contentWidth,
	})
	Object.defineProperty(doc.documentElement, 'clientWidth', {
		configurable: true,
		value: pageWidth,
	})
}

const COVER =
	'<h1 class="oculto" title="Cubierta"></h1><div class="cubierta"><svg viewBox="0 0 600 900" height="100%" width="100%"><image width="600" height="900" href="cover.jpg"></image></svg></div>'

describe('compensatedMediaCap', () => {
	it("divides ReadiumCSS's cap by the text zoom, so the drawn size is the cap again", () => {
		expect(compensatedMediaCap('95vh', 1.38)).toBe('calc(95vh / 1.38)')
		expect(compensatedMediaCap(' 95vh ', 0.8)).toBe('calc(95vh / 0.8)')
		expect(compensatedMediaCap('100vw', 2)).toBe('calc(100vw / 2)')
	})

	it('leaves the cap alone at 100% or without a cap', () => {
		expect(compensatedMediaCap('95vh', 1)).toBeNull()
		expect(compensatedMediaCap('', 1.38)).toBeNull()
		expect(compensatedMediaCap('none', 1.38)).toBeNull()
		expect(compensatedMediaCap('95vh', Number.NaN)).toBeNull()
	})
})

describe('isImageOnlyPage', () => {
	it('recognises a cover or an illustration page, not a page with text', () => {
		expect(isImageOnlyPage(frameDocument(COVER).body)).toBe(true)
		expect(isImageOnlyPage(frameDocument('<p><img src="plate.jpg"></p>').body)).toBe(true)
		expect(isImageOnlyPage(frameDocument('<p>Chapter 1</p><img src="a.jpg">').body)).toBe(false)
		expect(isImageOnlyPage(frameDocument('<h1></h1><p> </p>').body)).toBe(false)
	})
})

describe('syncFrameMedia', () => {
	it('caps the images for the text zoom, and only reports a change once', () => {
		const doc = frameDocument(COVER)
		setColumns(doc, 924, 924)

		expect(syncFrameMedia(doc, 1.38)).toBe(true)
		expect(doc.documentElement.style.getPropertyValue('--RS__maxMediaHeight')).toBe(
			'calc(95vh / 1.38)',
		)
		expect(syncFrameMedia(doc, 1.38)).toBe(false)

		// a bigger text size, then back to 100%
		expect(syncFrameMedia(doc, 2)).toBe(true)
		expect(doc.documentElement.style.getPropertyValue('--RS__maxMediaHeight')).toBe(
			'calc(95vh / 2)',
		)
		expect(syncFrameMedia(doc, 1)).toBe(true)
		expect(doc.documentElement.style.getPropertyValue('--RS__maxMediaHeight')).toBe('')
	})

	it('leaves fixed-layout pages (no ReadiumCSS cap) alone', () => {
		const doc = frameDocument(COVER, { cap: '' })
		expect(syncFrameMedia(doc, 1.38)).toBe(false)
		expect(doc.documentElement.getAttribute('style')).toBeNull()
		expect(doc.documentElement.hasAttribute(IMAGE_PAGE_ATTRIBUTE)).toBe(false)
	})

	it('drops the spacing of an image-only page that still spills to a second page, and keeps it so', () => {
		const doc = frameDocument(COVER)
		setColumns(doc, 1848, 924)
		expect(syncFrameMedia(doc, 1.38)).toBe(true)
		expect(doc.documentElement.hasAttribute(IMAGE_PAGE_ATTRIBUTE)).toBe(true)

		// it fits now: the attribute stays (without it the page would spill again)
		setColumns(doc, 924, 924)
		expect(syncFrameMedia(doc, 1.38)).toBe(false)
		expect(doc.documentElement.hasAttribute(IMAGE_PAGE_ATTRIBUTE)).toBe(true)
	})

	it('drops the spacing of an image-only page pushed to the second page of a spread', () => {
		const doc = frameDocument(COVER)
		// a two-page spread: the content stays within the spread, but on its second page
		doc.documentElement.style.columnCount = '2'
		setColumns(doc, 1344, 1344)
		vi.spyOn(doc.body, 'getBoundingClientRect').mockReturnValue({ right: 1300 } as DOMRect)
		syncFrameMedia(doc, 1.38)
		expect(doc.documentElement.hasAttribute(IMAGE_PAGE_ATTRIBUTE)).toBe(true)

		// on the spread's first page: left alone
		const first = frameDocument(COVER)
		first.documentElement.style.columnCount = '2'
		setColumns(first, 1344, 1344)
		vi.spyOn(first.body, 'getBoundingClientRect').mockReturnValue({ right: 660 } as DOMRect)
		syncFrameMedia(first, 1.38)
		expect(first.documentElement.hasAttribute(IMAGE_PAGE_ATTRIBUTE)).toBe(false)
	})

	it("doesn't touch pages with text, image pages that fit, or the scrolled layout", () => {
		const text = frameDocument('<p>Text</p><img src="a.jpg">')
		setColumns(text, 1848, 924)
		syncFrameMedia(text, 1.38)
		expect(text.documentElement.hasAttribute(IMAGE_PAGE_ATTRIBUTE)).toBe(false)

		const fits = frameDocument(COVER)
		setColumns(fits, 924, 924)
		syncFrameMedia(fits, 1.38)
		expect(fits.documentElement.hasAttribute(IMAGE_PAGE_ATTRIBUTE)).toBe(false)

		const scrolled = frameDocument(COVER, { style: '--USER__view: readium-scroll-on;' })
		setColumns(scrolled, 1848, 924)
		syncFrameMedia(scrolled, 1.38)
		expect(scrolled.documentElement.hasAttribute(IMAGE_PAGE_ATTRIBUTE)).toBe(false)
	})
})
