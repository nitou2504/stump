import { ReadingImageScaleFit } from '@stump/graphql'

import {
	backgroundColor,
	buildViews,
	canSplit,
	entryPos,
	fitSize,
	itemBox,
	stepView,
	swipeOf,
	tapZone,
	viewOfPage,
	wideZoomSize,
} from '../pagedView'

const phone = { w: 390, h: 844 }

describe('stepView: the next part of a page bigger than the screen (KOReader)', () => {
	// a tall page at fit width on a phone: 390 wide, 1600 tall
	const tall = { w: 390, h: 1600 }

	it('steps down a fit-width page with 15% overlap, then turns', () => {
		const step1 = stepView({
			pos: { x: 0, y: 0 },
			content: tall,
			viewport: phone,
			toward: 1,
			forward: true,
		})
		expect(step1).toEqual({ x: 0, y: 844 * 0.85 })
		const step2 = stepView({
			pos: step1 as { x: number; y: number },
			content: tall,
			viewport: phone,
			toward: 1,
			forward: true,
		})
		// clamped to the bottom (1600 - 844 = 756)
		expect(step2).toEqual({ x: 0, y: 756 })
		expect(
			stepView({ pos: { x: 0, y: 756 }, content: tall, viewport: phone, toward: 1, forward: true }),
		).toBe('turn')
	})

	it('goes back up the page before turning back', () => {
		const back = stepView({
			pos: { x: 0, y: 756 },
			content: tall,
			viewport: phone,
			toward: -1,
			forward: false,
		})
		expect(back).toEqual({ x: 0, y: 756 - 844 * 0.85 })
		expect(
			stepView({ pos: { x: 0, y: 0 }, content: tall, viewport: phone, toward: -1, forward: false }),
		).toBe('turn')
	})

	it('turns at once when the page fits the screen', () => {
		expect(
			stepView({
				pos: { x: 0, y: 0 },
				content: { w: 390, h: 600 },
				viewport: phone,
				toward: 1,
				forward: true,
			}),
		).toBe('turn')
	})

	// a wide page zoomed so half of it fills the width: 780 wide
	const wide = { w: 780, h: 569 }

	it('reads a wide page right to left in a manga: the right half first', () => {
		// entered going forward (leftward) at the right edge
		const start = entryPos(wide, phone, -1, true)
		expect(start).toEqual({ x: 390, y: 0 })
		// forward = leftward: x goes to the left half (clamped to 0), then the page turns
		const next = stepView({ pos: start, content: wide, viewport: phone, toward: -1, forward: true })
		expect(next).toEqual({ x: 390 - 390 * 0.85, y: 0 })
		const last = stepView({
			pos: { x: 0, y: 0 },
			content: wide,
			viewport: phone,
			toward: -1,
			forward: true,
		})
		expect(last).toBe('turn')
	})

	it('reads a wide page left to right otherwise', () => {
		const start = entryPos(wide, phone, 1, true)
		expect(start).toEqual({ x: 0, y: 0 })
		expect(
			stepView({ pos: start, content: wide, viewport: phone, toward: 1, forward: true }),
		).toEqual({
			x: 390 * 0.85,
			y: 0,
		})
	})

	it('walks a zoomed page row by row, right to left for manga (top-right, top-left, then down)', () => {
		const page = { w: 900, h: 1900 }
		// at the top-left of the top row: the next forward (leftward) step is the next row's right end
		const down = stepView({
			pos: { x: 0, y: 0 },
			content: page,
			viewport: phone,
			toward: -1,
			forward: true,
		})
		expect(down).toEqual({ x: 900 - 390, y: 844 * 0.85 })
		// in a left-to-right book the next row starts at the left
		const ltr = stepView({
			pos: { x: 510, y: 0 },
			content: page,
			viewport: phone,
			toward: 1,
			forward: true,
		})
		expect(ltr).toEqual({ x: 0, y: 844 * 0.85 })
	})

	it("enters a page coming back at its last part (KOReader's goto_end)", () => {
		const page = { w: 900, h: 1900 }
		// a left-to-right book, going back (leftward): the bottom-right corner
		expect(entryPos(page, phone, -1, false)).toEqual({ x: 510, y: 1900 - 844 })
		// a manga going back (rightward): the bottom-left corner
		expect(entryPos(page, phone, 1, false)).toEqual({ x: 0, y: 1900 - 844 })
	})
})

describe('swipeOf: a drag the page did not follow', () => {
	it('turns on a quick sideways flick', () => {
		expect(swipeOf(-60, 5, 120, 390)).toBe('left')
		expect(swipeOf(70, -10, 150, 390)).toBe('right')
	})

	it('turns on a slow drag across a quarter of the screen', () => {
		expect(swipeOf(-120, 0, 800, 390)).toBe('left')
	})

	it('ignores small, slow, diagonal or late drags', () => {
		expect(swipeOf(-10, 0, 50, 390)).toBeNull() // under the slop
		expect(swipeOf(-40, 0, 800, 390)).toBeNull() // slow and short
		expect(swipeOf(-80, 60, 120, 390)).toBeNull() // too diagonal
		expect(swipeOf(-300, 0, 1200, 390)).toBeNull() // held too long: not a swipe
	})
})

describe('tapZone', () => {
	it('uses thirds on touch screens and quarters with a mouse', () => {
		expect(tapZone(0.3, true)).toBe('left')
		expect(tapZone(0.3, false)).toBe('middle')
		expect(tapZone(0.7, true)).toBe('right')
		expect(tapZone(0.7, false)).toBe('middle')
		expect(tapZone(0.8, false)).toBe('right')
		expect(tapZone(0.5, true)).toBe('middle')
	})
})

describe('views: split wide pages', () => {
	const ratios: Record<number, number> = { 0: 0.7, 1: 1.37, 2: 0.7, 3: 2.6 }
	const ratioOf = (page: number) => ratios[page]

	it('splits a lone spread into its left and right halves, in screen order', () => {
		const views = buildViews([[0], [1], [2], [3]], ratioOf, true)
		expect(views).toEqual([
			[{ page: 0, part: 'whole' }],
			[{ page: 1, part: 'left' }],
			[{ page: 1, part: 'right' }],
			[{ page: 2, part: 'whole' }],
			// a panorama wider than 1.8 stays whole (KCC)
			[{ page: 3, part: 'whole' }],
		])
	})

	it('keeps pages whole without split, and never splits a two-page spread', () => {
		expect(buildViews([[0], [1]], ratioOf, false)).toEqual([
			[{ page: 0, part: 'whole' }],
			[{ page: 1, part: 'whole' }],
		])
		expect(buildViews([[0, 1]], ratioOf, true)).toEqual([
			[
				{ page: 0, part: 'whole' },
				{ page: 1, part: 'whole' },
			],
		])
	})

	it('finds the view of a page, its left half by default', () => {
		const views = buildViews([[0], [1], [2]], ratioOf, true)
		expect(viewOfPage(views, 1)).toBe(1)
		expect(viewOfPage(views, 1, 'right')).toBe(2)
		expect(viewOfPage(views, 2)).toBe(3)
		expect(viewOfPage(views, 9)).toBe(-1)
	})

	it('splits only spreads between 1.16 and 1.8 wide', () => {
		expect(canSplit(1.1)).toBe(false)
		expect(canSplit(1.37)).toBe(true)
		expect(canSplit(1.9)).toBe(false)
		expect(canSplit(undefined)).toBe(false)
	})

	it('boxes the halves and the crop', () => {
		const natural = { w: 1568, h: 1145 }
		expect(itemBox('left', natural)).toEqual({ x: 0, y: 0, w: 784, h: 1145 })
		expect(itemBox('right', natural)).toEqual({ x: 784, y: 0, w: 784, h: 1145 })
		const crop = { whole: { x: 10, y: 20, w: 1500, h: 1100 } }
		expect(itemBox('whole', natural, crop)).toEqual(crop.whole)
	})
})

describe('fitSize and wide pages', () => {
	const page = [{ w: 784, h: 1145 }] // ratio ~0.68

	it('fits a page to the screen, its width, its height or 1:1', () => {
		const screen = fitSize(page, ReadingImageScaleFit.Auto, phone)
		expect(screen.w).toBeCloseTo(390)
		const width = fitSize(page, ReadingImageScaleFit.Width, phone)
		expect(width.w).toBeCloseTo(390)
		const height = fitSize(page, ReadingImageScaleFit.Height, phone)
		expect(height.h).toBe(844)
		expect(fitSize(page, ReadingImageScaleFit.None, phone)).toEqual({ w: 784, h: 1145 })
	})

	it('puts the pages of a spread side by side at one height', () => {
		const spread = fitSize([page[0]!, page[0]!], ReadingImageScaleFit.Height, { w: 1440, h: 900 })
		expect(spread.h).toBe(900)
		expect(spread.w).toBeCloseTo((900 * 784 * 2) / 1145)
	})

	it('zooms a wide page so half of it fills the width', () => {
		const size = wideZoomSize(1568 / 1145, phone)
		expect(size.w).toBeCloseTo(780)
		// a very wide panorama: no taller than the screen
		expect(wideZoomSize(4, { w: 390, h: 100 }).h).toBe(100)
	})
})

describe('backgroundColor', () => {
	it("follows the setting, or the page's own margins for auto", () => {
		expect(backgroundColor('black', 'light')).toBe('#000000')
		expect(backgroundColor('white', 'dark')).toBe('#ffffff')
		expect(backgroundColor('auto', 'dark')).toBe('#000000')
		expect(backgroundColor('auto', 'light')).toBe('#ffffff')
		// no clear margin colour, or the theme: the reader theme's own paper
		expect(backgroundColor('auto', null)).toBeNull()
		expect(backgroundColor('theme', 'dark')).toBeNull()
	})
})
