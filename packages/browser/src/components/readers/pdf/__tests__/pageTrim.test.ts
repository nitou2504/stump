import type { PdfTextRun } from '@stump/sdk'

import { MIN_TRIM, shownY, textBox, TRIM_PAD, trimmedPage, trimRect } from '../pageTrim'

const run = (x: number, y: number, w: number, h: number, text: string): PdfTextRun => [
	x,
	y,
	w,
	h,
	text,
	0,
	1,
]

describe('trim margins', () => {
	it('boxes the text lines, leaving out stray marks', () => {
		const box = textBox([
			run(0.12, 0.1, 0.7, 0.02, 'No concept in mathematics'),
			run(0.12, 0.9, 0.05, 0.02, '10'),
			// a speck read as a character
			run(0.01, 0.99, 0.005, 0.005, '.'),
			run(0.97, 0.5, 0.01, 0.01, 'l'),
		])
		expect(box!.x).toBeCloseTo(0.12)
		expect(box!.y).toBeCloseTo(0.1)
		expect(box!.width).toBeCloseTo(0.7)
		expect(box!.height).toBeCloseTo(0.82)
		expect(textBox([])).toBeNull()
		expect(textBox(undefined)).toBeNull()
	})

	it('trims to the content, keeps the text inside and leaves a little paper', () => {
		// a page whose picture shows a page number below the detected content
		const trim = trimRect(
			{ x: 0.118, y: 0.064, width: 0.809, height: 0.88 },
			{ x: 0.12, y: 0.06, width: 0.8, height: 0.93 },
		)!
		expect(trim.x).toBeCloseTo(0.118 - TRIM_PAD)
		expect(trim.y).toBeCloseTo(0.06 - TRIM_PAD)
		expect(trim.x + trim.width).toBeCloseTo(0.927 + TRIM_PAD)
		// the text's last line (0.99) is kept, the page edge caps the paper
		expect(trim.y + trim.height).toBeCloseTo(1)
	})

	it('keeps the whole page when there is nothing worth trimming', () => {
		expect(trimRect(null, { x: 0.2, y: 0.2, width: 0.5, height: 0.5 })).toBeNull()
		const tiny = MIN_TRIM / 2
		expect(
			trimRect({ x: tiny, y: tiny, width: 1 - 2 * tiny, height: 1 - 2 * tiny }, null, 0),
		).toBeNull()
		// a nearly blank page doesn't collapse to a sliver
		expect(trimRect({ x: 0.5, y: 0.5, width: 0.01, height: 0.01 }, null)).toBeNull()
	})

	it('sizes the shown part and maps points down the page into it', () => {
		const trim = { x: 0.1, y: 0.2, width: 0.8, height: 0.5 }
		expect(trimmedPage([400, 600], trim)).toEqual([320, 300])
		expect(trimmedPage([400, 600], null)).toEqual([400, 600])
		expect(shownY(trim, 0.45)).toBeCloseTo(0.5)
		expect(shownY(null, 0.45)).toBe(0.45)
	})
})
