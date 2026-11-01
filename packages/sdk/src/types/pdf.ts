/**
 * Types for the PDF REST API (`/api/v2/pdf/{id}/…`), used by the web PDF reader.
 *
 * Coordinates are fractions (0-1) of the displayed page box (crop box, rotation applied),
 * origin top-left, so the page picture, the text runs and annotation rects line up at any
 * zoom.
 */

/** An entry of the PDF's outline (table of contents). `page` is 1-based, `null` if none */
export type PdfOutlineEntry = {
	title: string
	page: number | null
	children: PdfOutlineEntry[]
}

/** `GET /api/v2/pdf/{id}/layout` */
export type PdfLayout = {
	/** The document version; pass it as `v` to page and text URLs so they cache forever */
	v: string
	pageCount: number
	/** `[width, height]` per page in PDF points, rotation applied */
	pages: Array<[number, number]>
	/** Page labels when the PDF defines them (e.g. roman numerals), else `null` */
	labels: string[] | null
	outline: PdfOutlineEntry[]
	/** `false` when the sampled pages have no text (a scan without OCR) */
	hasText: boolean
	/** The picture widths (px) the server renders; a page URL's `w` snaps up to one of these */
	tiers: number[]
}

/**
 * One run of text on a page: `[x, y, width, height, text, firstChar, eol]`. The box is in
 * page fractions; `firstChar + i` is the PDFium char index of the run's i-th UTF-16 unit;
 * `eol` is 1 when a line end follows the run.
 */
export type PdfTextRun = [number, number, number, number, string, number, number]

/** `GET /api/v2/pdf/{id}/text/{n}` */
export type PdfPageText = {
	/** The page size in points (the same as the layout's) */
	w: number
	h: number
	runs: PdfTextRun[]
}

export type PdfRect = { x: number; y: number; width: number; height: number }

export type PdfSearchResult = {
	/** 1-based page */
	page: number
	snippet: string
	/** `[start, end)` of the hit inside `snippet` */
	match: [number, number]
	/** `[start, end)` PDFium char range on the page */
	chars: [number, number]
	rects: PdfRect[]
}

/** `GET /api/v2/pdf/{id}/search` */
export type PdfSearchResponse = {
	results: PdfSearchResult[]
	nextCursor: string | null
}

export type PdfSearchParams = {
	id: string
	q: string
	limit?: number
	cursor?: string
	signal?: AbortSignal
}

export type PdfPageURLParams = {
	/** The wanted picture width in device pixels; the server snaps it up to a tier */
	w: number
	/** The layout's document version */
	v?: string
	/** Low-priority request, which the server may drop (503) if the reader moved away */
	prefetch?: boolean
}
