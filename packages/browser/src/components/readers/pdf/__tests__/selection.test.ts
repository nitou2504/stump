import {
	boundaryToChar,
	locatorChars,
	readPdfSelection,
	selectionToLocator,
	totalProgression,
} from '../selection'
import { buildPageString } from '../textLayout'

/**
 * A page like the reader renders it: the page root with its number, and spans carrying the
 * PDFium index of their first char, with a <br> after line-ending runs
 */
function renderPage(page: number) {
	const root = document.createElement('div')
	root.setAttribute('data-pdf-page', String(page))
	root.innerHTML =
		'<img alt="">' +
		'<div class="pdf-text-layer">' +
		'<span data-char="100">The quick brown fox</span>' +
		'<span data-char="119"> jumps</span><br>' +
		'<span data-char="127">over the lazy dog.</span><br>' +
		'</div>'
	document.body.appendChild(root)
	return root
}

/** A DOMRect-like box (jsdom has no layout) */
const box = (x: number, y: number, width: number, height: number) =>
	({ x, y, width, height, left: x, top: y, right: x + width, bottom: y + height }) as DOMRect

const originalClientRects = Range.prototype.getClientRects
const originalBoundingRect = Range.prototype.getBoundingClientRect

afterEach(() => {
	document.body.innerHTML = ''
	window.getSelection()?.removeAllRanges()
	Range.prototype.getClientRects = originalClientRects
	Range.prototype.getBoundingClientRect = originalBoundingRect
	vi.restoreAllMocks()
})

describe('boundaryToChar', () => {
	it('adds the offset inside a span to its first char', () => {
		const page = renderPage(3)
		const fox = page.querySelector('[data-char="100"]')!.firstChild!
		expect(boundaryToChar(fox, 4, 'start')).toBe(104)
		expect(boundaryToChar(fox, 19, 'end')).toBe(119)
	})

	it('resolves boundaries between spans to the next start or the previous end', () => {
		const page = renderPage(3)
		const layer = page.querySelector('.pdf-text-layer')!
		// before the third child (the <br> after " jumps")
		expect(boundaryToChar(layer, 2, 'start')).toBe(127)
		expect(boundaryToChar(layer, 2, 'end')).toBe(125)
	})

	it('is null outside a page', () => {
		const stray = document.createElement('p')
		stray.textContent = 'hello'
		document.body.appendChild(stray)
		expect(boundaryToChar(stray.firstChild!, 1, 'start')).toBeNull()
	})
})

describe('readPdfSelection', () => {
	it('reads the page, chars, text and rects of a selection', () => {
		const page = renderPage(7)
		vi.spyOn(page, 'getBoundingClientRect').mockReturnValue(box(100, 0, 400, 500))
		// jsdom has no layout: give the range boxes
		Range.prototype.getClientRects = vi.fn(
			() =>
				[
					box(140, 50, 200, 10),
					box(340, 51, 40, 9),
					box(140, 65, 100, 10),
				] as unknown as DOMRectList,
		)
		Range.prototype.getBoundingClientRect = vi.fn(() => box(140, 50, 240, 25))

		const fox = page.querySelector('[data-char="100"]')!.firstChild!
		const dog = page.querySelector('[data-char="127"]')!.firstChild!
		const range = document.createRange()
		range.setStart(fox, 10) // "brown…"
		range.setEnd(dog, 4) // "…over"
		const selection = window.getSelection()!
		selection.removeAllRanges()
		selection.addRange(range)

		const result = readPdfSelection(selection)
		expect(result).toMatchObject({
			page: 7,
			chars: [110, 131],
			text: 'brown fox jumps over',
			anchor: { x: 140, y: 50, width: 240, height: 25 },
		})
		expect(result!.rects).toEqual([
			{ x: 0.1, y: 0.1, width: 0.6, height: 0.02 },
			{ x: 0.1, y: 0.13, width: 0.25, height: 0.02 },
		])
	})

	it('ignores collapsed selections and selections outside pages', () => {
		expect(readPdfSelection(window.getSelection())).toBeNull()
		const stray = document.createElement('p')
		stray.textContent = 'not a pdf'
		document.body.appendChild(stray)
		const range = document.createRange()
		range.selectNodeContents(stray)
		window.getSelection()!.removeAllRanges()
		window.getSelection()!.addRange(range)
		expect(readPdfSelection(window.getSelection())).toBeNull()
	})
})

describe('selectionToLocator', () => {
	it('builds the PDF locator stored with an annotation', () => {
		const pageString = buildPageString([
			[0.1, 0.1, 0.5, 0.02, 'The quick brown fox', 100, 0],
			[0.62, 0.1, 0.2, 0.02, ' jumps', 119, 1],
		])
		const locator = selectionToLocator(
			{
				page: 24,
				chars: [110, 119],
				text: 'brown fox',
				rects: [{ x: 0.3, y: 0.1, width: 0.2, height: 0.02 }],
				anchor: { x: 0, y: 0, width: 0, height: 0 },
			},
			{ mediaId: 'm1', pageCount: 100, label: 'xix', chapterTitle: 'Introduction', pageString },
		)
		expect(locator).toEqual({
			href: 'page/24',
			type: 'application/pdf',
			title: 'xix',
			chapterTitle: 'Introduction',
			locations: {
				position: 24,
				progression: 0.1,
				totalProgression: 0.231,
				fragments: ['page=24', 'chars=110-119'],
				rects: [{ x: 0.3, y: 0.1, width: 0.2, height: 0.02 }],
			},
			text: { highlight: 'brown fox', before: 'The quick ', after: ' jumps' },
		})
		expect(locatorChars(locator.locations?.fragments)).toEqual([110, 119])
	})

	it('computes the total progression', () => {
		expect(totalProgression(1, 0, 10)).toBe(0)
		expect(totalProgression(10, 1, 10)).toBe(1)
		expect(totalProgression(5, 0.5, 0)).toBe(0)
		expect(locatorChars(['page=3'])).toBeNull()
	})
})
