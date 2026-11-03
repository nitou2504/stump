/**
 * Keeps a book's drop caps beside their paragraph's first lines.
 *
 * A drop cap is a big first letter floated at the start of a paragraph (e.g. ePubLibre's
 * `<span class="capital">L</span>`). Publishers size its float for their own line height,
 * often with negative margins (`margin: -.2em .05em -.55em 0; line-height: 1.25em`), so with
 * the reader's line height (1.5) the float ends after the first line while the letter
 * reaches into the second: the second line starts under the letter and is drawn over it.
 *
 * Each drop cap's float is resized to whole lines of its paragraph: its box spans the lines
 * the letter covers (its baseline sits on the last one's), so the text wraps around the whole
 * letter and the following line starts clear of it. In a paginated book, a paragraph whose
 * first line is the last of a page would leave its float on the next page (the float doesn't
 * fit): such a paragraph starts on the next page with its letter instead.
 *
 * `::first-letter` drop caps aren't handled: no book in the library sample used them.
 */

/** Set on a drop cap whose float was resized */
export const DROP_CAP_ATTRIBUTE = 'data-stump-drop-cap'
/**
 * Set on a paragraph moved to the next page so its drop cap stays beside it; holds its own
 * inline `margin-top`, restored before the page is measured again
 */
export const DROP_CAP_PUSHED_ATTRIBUTE = 'data-stump-drop-cap-pushed'

/** A font's box (what CSS lays a line out with) or a text's ink, above and below the baseline */
export type VerticalMetrics = { ascent: number; descent: number }

export type DropCapInput = {
	/** The paragraph's line height, in CSS px */
	lineHeight: number
	/** The paragraph's font box, and the height of its capitals (the ink of an 'H') */
	paragraph: VerticalMetrics & { capHeight: number }
	/** The drop cap's font box, at its size */
	cap: VerticalMetrics
	/** The drop cap's letters' ink */
	ink: VerticalMetrics
}

export type DropCapLayout = {
	/** The paragraph line the letter's baseline sits on (1: the first) */
	lines: number
	/** The lines its float takes: one more when the letter reaches below that line */
	boxLines: number
	/** The float's line height, which is its height: `boxLines` paragraph lines */
	lineHeight: number
	marginTop: number
	marginBottom: number
}

/** A drop cap reaches 2-3 lines; past this it's not a drop cap (or not a sensible one) */
const MAX_LINES = 8

/**
 * Where a drop cap's float goes: its box spans whole paragraph lines (`boxLines`), and the
 * letter's baseline sits on the baseline of line `lines`, which puts its top near the top of
 * the first line's capitals (the usual drop cap).
 *
 * In CSS a line's baseline sits half the leading below its top, plus the font's ascent: for
 * the paragraph `(L - (A + D)) / 2 + A`; the float holds one line of the letter's font with
 * line height `H`, so its baseline is `(H - (a + d)) / 2 + a` below its top. The float's top
 * margin moves that baseline onto the paragraph's; the bottom margin takes it back, so the
 * float's margin box is exactly `boxLines` lines (less half a pixel, so rounding never
 * narrows the next line too).
 */
export function dropCapLayout({
	lineHeight,
	paragraph,
	cap,
	ink,
}: DropCapInput): DropCapLayout | null {
	if (
		![lineHeight, paragraph.ascent, paragraph.descent, cap.ascent, cap.descent, ink.ascent].every(
			Number.isFinite,
		)
	) {
		return null
	}
	if (lineHeight <= 0 || ink.ascent <= 0) return null

	// The first line's baseline, below its top
	const firstBaseline = (lineHeight - (paragraph.ascent + paragraph.descent)) / 2 + paragraph.ascent
	// From the top of the first line's capitals down to the letter's baseline
	const lines = clamp(Math.round((ink.ascent - paragraph.capHeight) / lineHeight) + 1, 1, MAX_LINES)
	const baseline = (lines - 1) * lineHeight + firstBaseline
	// A descender (a 'Q', a swash) reaching past the line's bottom takes the next line too
	const room = lineHeight - firstBaseline
	const boxLines = Math.min(
		MAX_LINES + 1,
		lines + Math.max(0, Math.ceil((ink.descent - room) / lineHeight)),
	)
	const height = boxLines * lineHeight
	const capBaseline = (height - (cap.ascent + cap.descent)) / 2 + cap.ascent
	const marginTop = baseline - capBaseline
	return {
		lines,
		boxLines,
		lineHeight: height,
		marginTop,
		marginBottom: -marginTop - 0.5,
	}
}

/** The blocks a drop cap opens */
const PARAGRAPHS = 'p, div, li, dd, blockquote'
/** What a drop cap holds: one to three characters, e.g. «L or 1 */
const MAX_CAP_LENGTH = 3
/** How much bigger than its paragraph's text a float's letter must be to be a drop cap */
const MIN_SIZE_RATIO = 1.4
const MEDIA = 'img, svg, image, video, canvas, object, embed'

/**
 * The drop caps of a document: a float opening a paragraph, holding a short text (no image)
 * at least 1.4× the paragraph's text size. A small float before it (an opening quote) may
 * come first.
 */
export function findDropCaps(doc: Document): { cap: HTMLElement; paragraph: HTMLElement }[] {
	const view = doc.defaultView
	const body = doc.body
	if (!view || !body) return []

	const found: { cap: HTMLElement; paragraph: HTMLElement }[] = []
	for (const paragraph of body.querySelectorAll<HTMLElement>(PARAGRAPHS)) {
		for (const candidate of leadingElements(paragraph)) {
			// a letter inside a nested paragraph is that paragraph's
			if (candidate.parentElement?.closest(PARAGRAPHS) !== paragraph) continue
			if (!isDropCap(candidate, paragraph, view)) continue
			found.push({ cap: candidate, paragraph })
			break
		}
	}
	return found
}

/**
 * The elements at a paragraph's start, before any of its text: its first two element children
 * (a quote float can precede the letter) and the first one's first child (a letter wrapped in
 * a `<b>` or `<a>`). A paragraph that starts with text has none.
 */
function leadingElements(paragraph: HTMLElement): HTMLElement[] {
	const elements: HTMLElement[] = []
	let textBefore = ''
	for (const node of paragraph.childNodes) {
		if (node.nodeType === Node.TEXT_NODE) {
			// the paragraph's text has started
			if ((node.textContent ?? '').trim().length > 0) break
			continue
		}
		if (node.nodeType !== Node.ELEMENT_NODE) continue
		const element = node as HTMLElement
		elements.push(element)
		// (not `instanceof HTMLElement`: the book's elements belong to its frame's window)
		const inner = element.firstElementChild as HTMLElement | null
		if (inner && (element.firstChild === inner || !element.firstChild?.textContent?.trim())) {
			elements.push(inner)
		}
		// a float before the letter (an opening quote) holds a character or two
		textBefore += element.textContent ?? ''
		if (elements.length >= 3 || textBefore.trim().length > MAX_CAP_LENGTH + 2) break
	}
	return elements
}

function isDropCap(element: HTMLElement, paragraph: HTMLElement, view: Window): boolean {
	const text = (element.textContent ?? '').trim()
	if (!text || [...text].length > MAX_CAP_LENGTH) return false
	if (element.querySelector(MEDIA)) return false
	const style = view.getComputedStyle(element)
	if (!isFloating(style.float)) return false
	const size = Number.parseFloat(style.fontSize)
	const paragraphSize = Number.parseFloat(view.getComputedStyle(paragraph).fontSize)
	return size > 0 && paragraphSize > 0 && size >= paragraphSize * MIN_SIZE_RATIO
}

const isFloating = (float: string) => float !== '' && float !== 'none'

/** What `measure` returns for a font and a text, in CSS px */
export type TextMeasure = { font: VerticalMetrics; ink: VerticalMetrics }
export type MeasureText = (font: string, size: number, text: string) => TextMeasure

/**
 * Measures text with a canvas of the book's own document, so the book's fonts (loaded there)
 * are the ones measured. A browser without the font box metrics gets a typical font's.
 */
export function canvasMeasure(doc: Document): MeasureText | null {
	const context = doc.createElement('canvas').getContext?.('2d')
	if (!context) return null
	const cache = new Map<string, TextMeasure>()
	return (font, size, text) => {
		const key = `${font}|${text}`
		const cached = cache.get(key)
		if (cached) return cached
		context.font = font
		const metrics = context.measureText(text)
		const measured: TextMeasure = {
			font: {
				ascent: finiteOr(metrics.fontBoundingBoxAscent, size * 0.9),
				descent: finiteOr(metrics.fontBoundingBoxDescent, size * 0.25),
			},
			ink: {
				ascent: finiteOr(metrics.actualBoundingBoxAscent, size * 0.7),
				descent: Math.max(0, finiteOr(metrics.actualBoundingBoxDescent, 0)),
			},
		}
		cache.set(key, measured)
		return measured
	}
}

const finiteOr = (value: number | undefined, fallback: number) =>
	typeof value === 'number' && Number.isFinite(value) ? value : fallback

/** The canvas font shorthand of an element's computed font */
function fontOf(style: CSSStyleDeclaration): string {
	return `${style.fontStyle} ${style.fontWeight} ${style.fontSize} ${style.fontFamily}`
}

/** An element's line height in px ('normal' is about its font's box) */
function lineHeightOf(style: CSSStyleDeclaration, fontBox: VerticalMetrics): number {
	const value = Number.parseFloat(style.lineHeight)
	return Number.isFinite(value) && style.lineHeight.endsWith('px')
		? value
		: fontBox.ascent + fontBox.descent
}

const px = (value: number) => `${Math.round(value * 100) / 100}px`

/** Sets an inline `!important` declaration; returns whether it changed */
function setImportant(element: HTMLElement, property: string, value: string): boolean {
	if (
		element.style.getPropertyValue(property) === value &&
		element.style.getPropertyPriority(property) === 'important'
	) {
		return false
	}
	element.style.setProperty(property, value, 'important')
	return true
}

/**
 * Resizes every drop cap's float of a document to whole lines of its paragraph; returns
 * whether anything changed. `measure` is for tests (jsdom has no canvas).
 */
export function syncDropCaps(
	doc: Document,
	measure: MeasureText | null = canvasMeasure(doc),
): boolean {
	const view = doc.defaultView
	if (!view || !measure) return false

	let changed = false
	for (const { cap, paragraph } of findDropCaps(doc)) {
		const capStyle = view.getComputedStyle(cap)
		const paragraphStyle = view.getComputedStyle(paragraph)
		const capSize = Number.parseFloat(capStyle.fontSize)
		const paragraphSize = Number.parseFloat(paragraphStyle.fontSize)
		const capText = (cap.textContent ?? '').trim()

		const paragraphFont = fontOf(paragraphStyle)
		const paragraphBox = measure(paragraphFont, paragraphSize, 'H')
		const letter = measure(fontOf(capStyle), capSize, capText)
		const layout = dropCapLayout({
			lineHeight: lineHeightOf(paragraphStyle, paragraphBox.font),
			paragraph: { ...paragraphBox.font, capHeight: paragraphBox.ink.ascent },
			cap: letter.font,
			ink: letter.ink,
		})
		if (!layout) continue

		// The float's height is its one line: a publisher's fixed height (often `1em`) goes
		changed = setImportant(cap, 'line-height', px(layout.lineHeight)) || changed
		changed = setImportant(cap, 'height', 'auto') || changed
		changed = setImportant(cap, 'margin-top', px(layout.marginTop)) || changed
		changed = setImportant(cap, 'margin-bottom', px(layout.marginBottom)) || changed
		changed = setImportant(cap, 'padding-top', '0px') || changed
		changed = setImportant(cap, 'padding-bottom', '0px') || changed
		if (capStyle.position === 'relative') {
			// a nudge would move the letter off the lines it was fitted to
			changed = setImportant(cap, 'top', 'auto') || changed
			changed = setImportant(cap, 'bottom', 'auto') || changed
		}
		if (!cap.hasAttribute(DROP_CAP_ATTRIBUTE)) {
			cap.setAttribute(DROP_CAP_ATTRIBUTE, String(layout.boxLines))
			changed = true
		} else if (cap.getAttribute(DROP_CAP_ATTRIBUTE) !== String(layout.boxLines)) {
			cap.setAttribute(DROP_CAP_ATTRIBUTE, String(layout.boxLines))
		}
		// A paragraph that opens with a drop cap isn't indented: the letter is its indent
		if (Number.parseFloat(paragraphStyle.textIndent) !== 0) {
			changed = setImportant(paragraph, 'text-indent', '0px') || changed
		}
	}
	return changed
}

/** Whether Readium lays the document out as pages (CSS columns), not as one scrolling column */
function isPaginated(root: HTMLElement): boolean {
	if ((root.getAttribute('style') ?? '').includes('readium-scroll-on')) return false
	const columns = Number.parseInt(
		root.ownerDocument.defaultView?.getComputedStyle(root).columnCount ?? '',
		10,
	)
	return Number.isFinite(columns) && columns >= 1
}

/** The page (column) a box starts on */
function columnOf(rect: DOMRect, pitch: number, scrollLeft: number): number {
	return Math.floor((rect.left + scrollLeft + 1) / pitch)
}

/** The box of a paragraph's first character after its drop cap (its first line) */
function firstLineRect(paragraph: HTMLElement, cap: HTMLElement): DOMRect | null {
	const doc = paragraph.ownerDocument
	const walker = doc.createTreeWalker(paragraph, NodeFilter.SHOW_TEXT)
	let node: Node | null
	while ((node = walker.nextNode())) {
		if (cap.contains(node)) continue
		const text = node.textContent ?? ''
		const index = text.search(/\S/)
		if (index < 0) continue
		const range = doc.createRange()
		range.setStart(node, index)
		range.setEnd(node, index + 1)
		const rect = range.getBoundingClientRect()
		if (rect.width > 0 || rect.height > 0) return rect
	}
	return null
}

/**
 * Starts on the next page any paragraph whose drop cap was left there without it: when the
 * paragraph's first line is the last of a page, its float (taller than a line) doesn't fit
 * and goes to the next page, leaving that line without its letter. A top margin as tall as
 * the page moves the paragraph to the next page; margins at a page break are dropped, so it
 * starts at that page's top. Returns whether anything changed.
 */
export function keepDropCapsWithParagraphs(doc: Document): boolean {
	const root = doc.documentElement
	const view = doc.defaultView
	if (!root || !view) return false

	// Measure from scratch: a paragraph moved before may fit where it was now
	const before = new Set(doc.querySelectorAll<HTMLElement>(`[${DROP_CAP_PUSHED_ATTRIBUTE}]`))
	for (const paragraph of before) {
		const own = paragraph.getAttribute(DROP_CAP_PUSHED_ATTRIBUTE) ?? ''
		if (own) paragraph.style.setProperty('margin-top', own)
		else paragraph.style.removeProperty('margin-top')
		paragraph.removeAttribute(DROP_CAP_PUSHED_ATTRIBUTE)
	}

	const after = new Set<HTMLElement>()
	const columns = Number.parseInt(view.getComputedStyle(root).columnCount, 10)
	const pitch = root.clientWidth / Math.max(1, columns)
	if (isPaginated(root) && pitch > 0) {
		const scrollLeft = doc.scrollingElement?.scrollLeft ?? root.scrollLeft
		// the paragraph's CSS px are zoomed with the text: a page's height, in them
		const pageHeight = root.clientHeight / bodyZoom(doc)
		for (const cap of doc.querySelectorAll<HTMLElement>(`[${DROP_CAP_ATTRIBUTE}]`)) {
			const paragraph = cap.parentElement?.closest<HTMLElement>(PARAGRAPHS)
			if (!paragraph || after.has(paragraph)) continue
			const line = firstLineRect(paragraph, cap)
			if (!line) continue
			// the first line was left on an earlier page than its letter
			if (
				columnOf(line, pitch, scrollLeft) >=
				columnOf(cap.getBoundingClientRect(), pitch, scrollLeft)
			) {
				continue
			}
			paragraph.setAttribute(
				DROP_CAP_PUSHED_ATTRIBUTE,
				paragraph.style.getPropertyValue('margin-top'),
			)
			paragraph.style.setProperty('margin-top', px(pageHeight), 'important')
			after.add(paragraph)
		}
	}
	return before.size !== after.size || [...after].some((paragraph) => !before.has(paragraph))
}

/** The zoom Readium applies to the book's body (1 when it uses font-size or nothing) */
function bodyZoom(doc: Document): number {
	const body = doc.body
	const view = doc.defaultView
	if (!body || !view) return 1
	const zoom = Number.parseFloat(view.getComputedStyle(body).zoom)
	return Number.isFinite(zoom) && zoom > 0 ? zoom : 1
}

const attached = new WeakSet<Document>()

/**
 * Keeps a frame's drop caps fitted from now on: when it loads, when its fonts arrive (the
 * letter's font changes its size) and each time Readium restyles the page (a new line height,
 * font, text size or layout rewrites `<html>`'s style). `onLayoutChange` runs when the layout
 * changed, so page counts can be measured again.
 */
export function fitDropCaps(doc: Document, onLayoutChange?: () => void) {
	const root = doc.documentElement
	const view = doc.defaultView
	if (!root || !view || attached.has(doc)) return
	attached.add(doc)

	const sync = () => {
		try {
			// a new canvas each time: fonts may have arrived since
			const resized = syncDropCaps(doc, canvasMeasure(doc))
			// only books with drop caps pay for the page check
			const moved = doc.querySelector(`[${DROP_CAP_ATTRIBUTE}]`)
				? keepDropCapsWithParagraphs(doc)
				: false
			if (resized || moved) onLayoutChange?.()
		} catch {
			// The frame was torn down
		}
	}
	sync()

	const observer = new view.MutationObserver(sync)
	observer.observe(root, { attributes: true, attributeFilter: ['style'] })
	// the letter's font (often the book's own) and images above it load after the page
	view.addEventListener('load', sync, { once: true })
	doc.fonts?.ready.then(sync).catch(() => {})
	doc.fonts?.addEventListener?.('loadingdone', sync)
}

function clamp(value: number, min: number, max: number) {
	return Math.min(max, Math.max(min, value))
}
