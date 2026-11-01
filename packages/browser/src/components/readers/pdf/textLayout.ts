import type { PdfTextRun } from '@stump/sdk'

/**
 * The text layer's font: one generic family, measured once per run, so the spans can be
 * stretched (`scaleX`) to the width the run has on the page.
 */
export const TEXT_LAYER_FONT_FAMILY = 'sans-serif'

/** Measures a text's width at a font size of 1 px */
export type MeasureText = (text: string) => number

let sharedContext: CanvasRenderingContext2D | null | undefined

/** A canvas-based measure (100 px reference size, so rounding doesn't matter) */
export function canvasMeasureText(): MeasureText | null {
	if (sharedContext === undefined) {
		try {
			sharedContext = document.createElement('canvas').getContext('2d')
		} catch {
			sharedContext = null
		}
		if (sharedContext) {
			sharedContext.font = `100px ${TEXT_LAYER_FONT_FAMILY}`
		}
	}
	const context = sharedContext
	if (!context) return null
	return (text: string) => context.measureText(text).width / 100
}

/**
 * How a run is laid out over the page, in page-relative units that don't depend on the
 * zoom: the font size as a fraction of the page height and the horizontal stretch that
 * makes the text as wide as the run's box.
 */
export type RunLayout = {
	left: number
	top: number
	/** Font size as a fraction of the page height */
	fontSize: number
	scaleX: number
	text: string
	firstChar: number
	eol: boolean
}

/**
 * Lays out a page's runs. `aspect` is the page's width / height (points). The stretch is
 * `(run width × page width) / (text width at 1 px × run height × page height)`, so it is the
 * same at every zoom.
 */
export function layoutRuns(
	runs: PdfTextRun[],
	aspect: number,
	measure: MeasureText | null,
): RunLayout[] {
	return runs.map(([x, y, width, height, text, firstChar, eol]) => {
		const measured = measure ? measure(text) : 0
		const naturalWidth = measured * height
		const targetWidth = width * aspect
		const scaleX = naturalWidth > 0 ? targetWidth / naturalWidth : 1
		return {
			left: x,
			top: y,
			fontSize: height,
			scaleX: Number.isFinite(scaleX) && scaleX > 0 ? scaleX : 1,
			text,
			firstChar,
			eol: eol === 1,
		}
	})
}

/**
 * A page's text as one string, the way it is copied (line ends as `\n`), with a map from
 * PDFium char indexes to positions in that string.
 */
export type PageString = {
	text: string
	/** The string offset of a PDFium char index (the nearest char at or after it) */
	offsetOf: (charIndex: number) => number
}

export function buildPageString(runs: PdfTextRun[]): PageString {
	let text = ''
	const starts: Array<{ firstChar: number; offset: number; length: number }> = []
	for (const [, , , , runText, firstChar, eol] of runs) {
		starts.push({ firstChar, offset: text.length, length: runText.length })
		text += runText
		if (eol === 1) text += '\n'
	}
	const sorted = [...starts].sort((a, b) => a.firstChar - b.firstChar)

	const offsetOf = (charIndex: number) => {
		for (const run of sorted) {
			if (charIndex < run.firstChar) return run.offset
			if (charIndex < run.firstChar + run.length) return run.offset + (charIndex - run.firstChar)
		}
		return text.length
	}

	return { text, offsetOf }
}

/** Up to `size` characters before and after a char range, whitespace collapsed */
export function quoteContext(
	page: PageString,
	start: number,
	end: number,
	size = 32,
): { before: string; after: string } {
	const from = page.offsetOf(start)
	const to = page.offsetOf(end)
	const before = collapseWhitespace(page.text.slice(Math.max(0, from - size), from), 'start')
	const after = collapseWhitespace(page.text.slice(to, to + size), 'end')
	return { before, after }
}

/** The first `length` characters of a page's text, for a bookmark preview */
export function pagePreview(runs: PdfTextRun[], length = 80): string {
	const text = collapseWhitespace(buildPageString(runs).text, 'both')
	return text.length > length ? `${text.slice(0, length - 1).trimEnd()}…` : text
}

export function collapseWhitespace(text: string, trim: 'start' | 'end' | 'both' | 'none' = 'both') {
	const collapsed = text.replace(/\s+/g, ' ')
	switch (trim) {
		case 'start':
			return collapsed.trimStart()
		case 'end':
			return collapsed.trimEnd()
		case 'both':
			return collapsed.trim()
		default:
			return collapsed
	}
}
