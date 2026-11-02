import type { PdfTextRun } from '@stump/sdk'

import {
	buildPageString,
	collapseWhitespace,
	layoutRuns,
	pagePreview,
	quoteContext,
} from '../textLayout'

const runs: PdfTextRun[] = [
	[0.1, 0.1, 0.5, 0.02, 'The quick brown fox', 100, 0],
	[0.62, 0.1, 0.2, 0.02, ' jumps', 119, 1],
	[0.1, 0.13, 0.4, 0.02, 'over the lazy dog.', 127, 1],
]

describe('layoutRuns', () => {
	it('stretches each run to its box, independent of the zoom', () => {
		// a fake font where every char is 0.5 px wide at 1 px
		const measure = (text: string) => text.length * 0.5
		const [first] = layoutRuns(runs, 612 / 792, measure)
		// natural width = 19 chars * 0.5 * 0.02 = 0.19 (page heights); target = 0.5 * aspect
		expect(first!.scaleX).toBeCloseTo((0.5 * (612 / 792)) / 0.19)
		expect(first).toMatchObject({ left: 0.1, top: 0.1, fontSize: 0.02, firstChar: 100, eol: false })
	})

	it('leaves runs unstretched without a measure', () => {
		expect(layoutRuns(runs, 1, null).every((run) => run.scaleX === 1)).toBe(true)
	})
})

describe('buildPageString', () => {
	const page = buildPageString(runs)

	it('joins the runs, with line ends where the PDF has them', () => {
		expect(page.text).toBe('The quick brown fox jumps\nover the lazy dog.\n')
	})

	it('maps PDFium char indexes to string offsets', () => {
		expect(page.offsetOf(100)).toBe(0)
		expect(page.offsetOf(104)).toBe(4)
		expect(page.offsetOf(119)).toBe(19)
		expect(page.offsetOf(127)).toBe(26)
		// a char between runs maps to the next run's start
		expect(page.offsetOf(126)).toBe(26)
		expect(page.offsetOf(9999)).toBe(page.text.length)
	})

	it('gives the context around a quote', () => {
		// "brown fox" = chars 110-119
		expect(quoteContext(page, 110, 119, 10)).toEqual({ before: 'The quick ', after: ' jumps ove' })
	})
})

describe('previews', () => {
	it('shortens a page to its first words', () => {
		expect(pagePreview(runs, 20)).toBe('The quick brown fox…')
		expect(pagePreview(runs)).toBe('The quick brown fox jumps over the lazy dog.')
	})

	it('collapses whitespace', () => {
		expect(collapseWhitespace('  a \n b  ')).toBe('a b')
		expect(collapseWhitespace('  a \n b  ', 'start')).toBe('a b ')
	})
})
