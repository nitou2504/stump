import {
	chapterAround,
	chapterLabel,
	chapterPages,
	MIN_TICK_GAP,
	READING_PROGRESS_DEFAULTS,
	resolveReadingProgress,
	thinTicks,
	tocPath,
} from '../readingProgress'

describe('reading progress settings', () => {
	it('shows the bar in the EPUB reader and the number elsewhere until changed', () => {
		expect(resolveReadingProgress('epub', undefined)).toEqual({
			display: 'bar',
			items: ['chapter', 'pagesLeft', 'percent'],
			show: 'always',
		})
		expect(resolveReadingProgress('pdf', undefined).display).toBe('number')
		expect(resolveReadingProgress('comic', undefined).display).toBe('number')
		expect(READING_PROGRESS_DEFAULTS.comic.items).toEqual(['page'])
	})

	it('keeps what was picked and drops items a reader cannot show', () => {
		expect(resolveReadingProgress('epub', { display: 'off' }).display).toBe('off')
		expect(resolveReadingProgress('epub', { show: 'withControls' }).show).toBe('withControls')
		// a comic has no chapters
		expect(
			resolveReadingProgress('comic', { items: ['chapter', 'page', 'pagesLeft', 'clock'] }).items,
		).toEqual(['page', 'clock'])
	})
})

describe('the reading bar rule', () => {
	it('draws a tick at each chapter start, but not at the book start or end', () => {
		expect(thinTicks([0, 0.25, 0.5, 1], 680)).toEqual([0.25, 0.5])
	})

	it('thins ticks closer than 6px, keeping the earlier one', () => {
		// on a 100px rule, 0.10 and 0.13 are 3px apart
		expect(MIN_TICK_GAP).toBe(6)
		expect(thinTicks([0.1, 0.13, 0.2, 0.21, 0.5], 100)).toEqual([0.1, 0.2, 0.5])
		// the same book on a wide rule keeps them all
		expect(thinTicks([0.1, 0.13, 0.2, 0.21, 0.5], 1000)).toEqual([0.1, 0.13, 0.2, 0.21, 0.5])
	})

	it('finds the chapter around the position (the thick segment)', () => {
		const starts = [0, 0.2, 0.45, 0.8]
		expect(chapterAround(starts, 0.1)).toEqual([0, 0.2])
		expect(chapterAround(starts, 0.47)).toEqual([0.45, 0.8])
		expect(chapterAround(starts, 0.9)).toEqual([0.8, 1])
		// a book whose contents don't start at its beginning
		expect(chapterAround([0.1, 0.5], 0.05)).toEqual([0, 0.1])
	})
})

describe('pages left in the chapter', () => {
	it('counts screens across a chapter whose pages turn', () => {
		// 8 screens of 400px; on the third
		expect(chapterPages({ scrollSize: 3200, viewportSize: 400, offset: 800 })).toEqual({
			page: 3,
			pages: 8,
			left: 5,
		})
		// a spread is one screen (two columns): the same chapter in a 800px spread
		expect(chapterPages({ scrollSize: 3200, viewportSize: 800, offset: 800 })).toEqual({
			page: 2,
			pages: 4,
			left: 2,
		})
	})

	it('reads right-to-left books scrolled to negative offsets', () => {
		expect(chapterPages({ scrollSize: 2000, viewportSize: 400, offset: -1600 })).toEqual({
			page: 5,
			pages: 5,
			left: 0,
		})
	})

	it('counts screen heights down a scrolled chapter', () => {
		expect(chapterPages({ scrollSize: 5000, viewportSize: 1000, offset: 2400 })).toEqual({
			page: 3,
			pages: 5,
			left: 2,
		})
	})

	it('copes with a chapter shorter than the screen, and with nothing to measure', () => {
		expect(chapterPages({ scrollSize: 300, viewportSize: 400, offset: 0 })).toEqual({
			page: 1,
			pages: 1,
			left: 0,
		})
		expect(chapterPages({ scrollSize: 0, viewportSize: 400, offset: 0 })).toBeNull()
	})
})

describe("the chapter's name", () => {
	const toc = [
		{ label: 'Book I', content: 'b1.xhtml', children: [] },
		{
			label: 'Book III ',
			content: 'b3.xhtml',
			children: [
				{ label: 'Chapter 1', content: 'b3c1.xhtml', children: [] },
				{ label: 'Chapter 2', content: 'b3c2.xhtml', children: [] },
			],
		},
	]

	it('names a chapter after its part, as "Book III · Chapter 1"', () => {
		const path = tocPath(toc, (item) => item.content === 'b3c1.xhtml')
		expect(path).toEqual(['Book III', 'Chapter 1'])
		expect(chapterLabel(path)).toBe('Book III · Chapter 1')
	})

	it('names a top-level chapter by itself, and nothing when it is not in the contents', () => {
		expect(chapterLabel(tocPath(toc, (item) => item.content === 'b1.xhtml'))).toBe('Book I')
		expect(chapterLabel(tocPath(toc, () => false))).toBeUndefined()
	})
})
