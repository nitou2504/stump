import {
	DROP_CAP_ATTRIBUTE,
	DROP_CAP_PUSHED_ATTRIBUTE,
	dropCapLayout,
	findDropCaps,
	keepDropCapsWithParagraphs,
	type MeasureText,
	syncDropCaps,
} from '../fitDropCaps'

/** A frame document holding the given body (an iframe's document has a window, like the reader's) */
function frameDocument(bodyHtml: string, rootStyle = '') {
	const iframe = document.createElement('iframe')
	document.body.appendChild(iframe)
	const doc = iframe.contentDocument!
	if (rootStyle) doc.documentElement.setAttribute('style', rootStyle)
	doc.body.innerHTML = bodyHtml
	return doc
}

afterEach(() => {
	document.body.innerHTML = ''
	vi.restoreAllMocks()
})

/** A typical serif: a box of 0.9 + 0.25 of the size, capitals 0.7 tall */
const measure: MeasureText = (_font, size, text) => ({
	font: { ascent: size * 0.9, descent: size * 0.25 },
	ink: { ascent: size * 0.7, descent: text === 'Q' ? size * 0.2 : 0 },
})

/** Moby Dick's chapter openings (ePubLibre): a 3em Castellar 'L' sized for another line height */
const MOBY = `<h1>CAPÍTULO I</h1><p class="asangre" style="font-size: 16px; line-height: 24px; text-indent: 24px"><span class="capital" style="float: left; font-size: 48px; line-height: 60px; margin: -9.6px 0.8px -26.4px 0">L</span>lamadme Ismael. Hace unos años…</p><p style="font-size: 16px">Ahí está vuestra ciudad insular…</p>`

describe('dropCapLayout', () => {
	const paragraph = { ascent: 14.4, descent: 4, capHeight: 11.2 }

	it("puts a 3× letter's baseline on the second line and its float over exactly two lines", () => {
		const layout = dropCapLayout({
			lineHeight: 24,
			paragraph,
			cap: { ascent: 43.2, descent: 12 },
			ink: { ascent: 33.6, descent: 0 },
		})!
		expect(layout.lines).toBe(2)
		expect(layout.boxLines).toBe(2)
		expect(layout.lineHeight).toBe(48)
		// the float's margin box is two lines (less half a pixel): the third line starts clear
		expect(layout.marginTop + layout.lineHeight + layout.marginBottom).toBeCloseTo(47.5)
		// its baseline lands on the second line's: one line plus the first line's baseline
		const firstBaseline = (24 - (14.4 + 4)) / 2 + 14.4
		const capBaseline = (48 - (43.2 + 12)) / 2 + 43.2
		expect(layout.marginTop + capBaseline).toBeCloseTo(24 + firstBaseline)
	})

	it('spans as many lines as the letter is tall', () => {
		const tall = dropCapLayout({
			lineHeight: 24,
			paragraph,
			cap: { ascent: 66, descent: 18 },
			ink: { ascent: 51.5, descent: 0 },
		})!
		expect(tall.lines).toBe(3)
		expect(tall.lineHeight).toBe(72)
		// a raised initial no taller than the text stays on its line
		const small = dropCapLayout({
			lineHeight: 24,
			paragraph,
			cap: { ascent: 21.6, descent: 6 },
			ink: { ascent: 16.8, descent: 0 },
		})!
		expect(small.lines).toBe(1)
	})

	it('takes one more line when the letter reaches below its last line', () => {
		const layout = dropCapLayout({
			lineHeight: 24,
			paragraph,
			cap: { ascent: 43.2, descent: 12 },
			ink: { ascent: 33.6, descent: 14 },
		})!
		expect(layout.lines).toBe(2)
		expect(layout.boxLines).toBe(3)
		expect(layout.lineHeight).toBe(72)
	})

	it('gives up on metrics it cannot use', () => {
		expect(
			dropCapLayout({
				lineHeight: 0,
				paragraph,
				cap: { ascent: 1, descent: 1 },
				ink: { ascent: 1, descent: 0 },
			}),
		).toBeNull()
		expect(
			dropCapLayout({
				lineHeight: 24,
				paragraph,
				cap: { ascent: Number.NaN, descent: 1 },
				ink: { ascent: 1, descent: 0 },
			}),
		).toBeNull()
	})
})

describe('findDropCaps', () => {
	it('finds a big floated letter opening a paragraph', () => {
		const doc = frameDocument(MOBY)
		const found = findDropCaps(doc)
		expect(found).toHaveLength(1)
		expect(found[0]!.cap.textContent).toBe('L')
		expect(found[0]!.paragraph.className).toBe('asangre')
	})

	it('finds it after an opening quote, or wrapped in another element', () => {
		const quoted = frameDocument(
			'<p style="font-size: 16px"><span style="float: left; font-size: 16px">«</span> <span style="float: left; font-size: 56px">T</span>odo empezó…</p>',
		)
		expect(findDropCaps(quoted).map(({ cap }) => cap.textContent)).toEqual(['T'])
		const wrapped = frameDocument(
			'<div><p style="font-size: 16px"><b><span style="float: left; font-size: 48px">E</span></b>ra una vez…</p></div>',
		)
		const found = findDropCaps(wrapped)
		expect(found.map(({ cap }) => cap.textContent)).toEqual(['E'])
		// the letter belongs to its own paragraph, not the block around it
		expect(found[0]!.paragraph.tagName).toBe('P')
	})

	it('leaves alone floats that are not drop caps', () => {
		const doc = frameDocument(
			[
				// an image floated beside the text
				'<p style="font-size: 16px"><span style="float: left; font-size: 48px"><img src="a.png"></span>Texto</p>',
				// a floated word, not a letter
				'<p style="font-size: 16px"><span style="float: right; font-size: 48px">Nota</span>Texto</p>',
				// a float the size of the text
				'<p style="font-size: 16px"><span style="float: left; font-size: 16px">A</span>Texto</p>',
				// a big letter that isn't floated (a raised initial)
				'<p style="font-size: 16px"><span style="font-size: 32px">B</span>ueno</p>',
				// a float after the paragraph's text has started
				'<p style="font-size: 16px">Texto <span style="float: left; font-size: 48px">C</span></p>',
			].join(''),
		)
		expect(findDropCaps(doc)).toEqual([])
	})
})

describe('syncDropCaps', () => {
	it("resizes the float to the paragraph's lines and drops the paragraph's indent", () => {
		const doc = frameDocument(MOBY)
		expect(syncDropCaps(doc, measure)).toBe(true)
		const cap = doc.querySelector<HTMLElement>('.capital')!
		const expected = dropCapLayout({
			lineHeight: 24,
			paragraph: { ascent: 14.4, descent: 4, capHeight: 11.2 },
			cap: { ascent: 43.2, descent: 12 },
			ink: { ascent: 33.6, descent: 0 },
		})!
		expect(cap.style.getPropertyValue('line-height')).toBe(`${expected.lineHeight}px`)
		expect(cap.style.getPropertyPriority('line-height')).toBe('important')
		expect(Number.parseFloat(cap.style.getPropertyValue('margin-top'))).toBeCloseTo(
			expected.marginTop,
			1,
		)
		expect(Number.parseFloat(cap.style.getPropertyValue('margin-bottom'))).toBeCloseTo(
			expected.marginBottom,
			1,
		)
		expect(cap.style.getPropertyValue('height')).toBe('auto')
		expect(cap.getAttribute(DROP_CAP_ATTRIBUTE)).toBe('2')
		expect(doc.querySelector<HTMLElement>('.asangre')!.style.getPropertyValue('text-indent')).toBe(
			'0px',
		)
	})

	it('changes nothing the second time, and nothing in a book without drop caps', () => {
		const doc = frameDocument(MOBY)
		syncDropCaps(doc, measure)
		expect(syncDropCaps(doc, measure)).toBe(false)
		expect(syncDropCaps(frameDocument('<p style="font-size: 16px">Texto</p>'), measure)).toBe(false)
	})
})

describe('keepDropCapsWithParagraphs', () => {
	/** A two-page spread 1000px wide and 800px tall; the letter and the first line at given x */
	function paginated(capLeft: number, lineLeft: number) {
		const doc = frameDocument(MOBY, 'column-count: 2')
		syncDropCaps(doc, measure)
		const root = doc.documentElement
		Object.defineProperty(root, 'clientWidth', { configurable: true, value: 1000 })
		Object.defineProperty(root, 'clientHeight', { configurable: true, value: 800 })
		const cap = doc.querySelector<HTMLElement>('.capital')!
		vi.spyOn(cap, 'getBoundingClientRect').mockReturnValue(new DOMRect(capLeft, 0, 40, 60))
		const view = doc.defaultView as Window & typeof globalThis
		view.Range.prototype.getBoundingClientRect = () => new DOMRect(lineLeft, 770, 10, 24)
		return doc
	}

	it('starts the paragraph on the next page when its letter was left there without it', () => {
		const doc = paginated(520, 20)
		expect(keepDropCapsWithParagraphs(doc)).toBe(true)
		const paragraph = doc.querySelector<HTMLElement>('.asangre')!
		expect(paragraph.hasAttribute(DROP_CAP_PUSHED_ATTRIBUTE)).toBe(true)
		// a page's height pushes it past the break, where the margin is dropped
		expect(paragraph.style.getPropertyValue('margin-top')).toBe('800px')
		// measured again, the same: nothing changes
		expect(keepDropCapsWithParagraphs(doc)).toBe(false)
	})

	it('leaves a paragraph whose letter is beside its first line', () => {
		const doc = paginated(20, 64)
		expect(keepDropCapsWithParagraphs(doc)).toBe(false)
		expect(doc.querySelector('.asangre')!.hasAttribute(DROP_CAP_PUSHED_ATTRIBUTE)).toBe(false)
	})

	it('gives a moved paragraph its own margin back when it fits again, or when scrolled', () => {
		const doc = paginated(520, 20)
		const paragraph = doc.querySelector<HTMLElement>('.asangre')!
		paragraph.style.setProperty('margin-top', '3px')
		keepDropCapsWithParagraphs(doc)
		expect(paragraph.getAttribute(DROP_CAP_PUSHED_ATTRIBUTE)).toBe('3px')
		// the scrolled layout has no pages to keep together
		doc.documentElement.setAttribute('style', 'column-count: auto; --USER__view: readium-scroll-on')
		expect(keepDropCapsWithParagraphs(doc)).toBe(true)
		expect(paragraph.style.getPropertyValue('margin-top')).toBe('3px')
		expect(paragraph.hasAttribute(DROP_CAP_PUSHED_ATTRIBUTE)).toBe(false)
	})
})
