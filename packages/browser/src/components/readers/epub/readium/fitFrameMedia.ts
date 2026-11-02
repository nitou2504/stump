/**
 * Keeps a book's images inside the page when Readium scales the text.
 *
 * Readium's font size is CSS `zoom` on the book's `<body>` (e.g. `zoom: 138%` for 22px), but
 * ReadiumCSS caps images at `--RS__maxMediaHeight` (95vh) measured before that zoom. So a
 * cover sized to the cap is drawn 1.38× taller than the page: it can't fit any column, and
 * after an empty heading (ePubLibre's hidden `<h1 class="oculto">`) it's pushed to the
 * chapter's second page, leaving the first one blank. Dividing the cap by the zoom makes the
 * drawn size the cap again.
 *
 * A page that holds only images (a cover, a plate) and still spills past its first page (or
 * to the second page of a two-page spread) gets its vertical margins and paddings dropped as
 * well, so the image starts the page whatever the text size (the empty heading's margin grows
 * with the zoom).
 */

/** Set on a frame's `<html>` when an image-only page needs its margins dropped to fit */
export const IMAGE_PAGE_ATTRIBUTE = 'data-stump-image-page'
/** Dispatched on the host window when a frame's layout changed (e.g. the reading bar re-measures) */
export const EPUB_FRAME_LAYOUT_EVENT = 'stump:epub-frame-layout'

const FIT_MEDIA_STYLE_ID = 'stump-fit-media'
const MEDIA_CAP_PROPERTY = '--RS__maxMediaHeight'
const MEDIA_SELECTOR = 'img, svg, video, image, object, embed, canvas'

/**
 * The media cap to set for a body zoom, from ReadiumCSS's own cap (e.g. `95vh`); `null` when
 * there's nothing to compensate (no zoom) or no cap to compensate (not a ReadiumCSS page)
 */
export function compensatedMediaCap(baseCap: string, zoom: number): string | null {
	const base = baseCap.trim()
	if (!base || base === 'none') return null
	if (!Number.isFinite(zoom) || zoom <= 0 || Math.abs(zoom - 1) < 0.001) return null
	return `calc(${base} / ${Number(zoom.toFixed(4))})`
}

/** Whether a document shows images and no text at all (a cover, an illustration page) */
export function isImageOnlyPage(body: HTMLElement | null): boolean {
	if (!body) return false
	if ((body.textContent ?? '').trim() !== '') return false
	return !!body.querySelector(MEDIA_SELECTOR)
}

/**
 * Whether a paginated frame's content runs past its first column: past the first page, or to
 * the second page of a two-page spread (the pages are CSS columns side by side)
 */
export function spillsPastFirstColumn(root: HTMLElement, body: HTMLElement | null): boolean {
	if (root.scrollWidth > root.clientWidth + 1) return true
	if (!body) return false
	const view = root.ownerDocument.defaultView
	const columns = Number.parseInt(view?.getComputedStyle(root).columnCount ?? '', 10)
	if (!Number.isFinite(columns) || columns < 2) return false
	// the body's box spans every column its content reaches
	return body.getBoundingClientRect().right > root.clientWidth / columns + 1
}

/** The zoom Readium applies to the book's body (1 when it uses font-size or nothing) */
function bodyZoom(doc: Document): number {
	const body = doc.body
	const view = doc.defaultView
	if (!body || !view) return 1
	const zoom = Number.parseFloat(view.getComputedStyle(body).zoom)
	return Number.isFinite(zoom) && zoom > 0 ? zoom : 1
}

const baseCaps = new WeakMap<Document, string>()

/**
 * Applies the compensation to a frame's document now; returns whether its layout changed.
 * Exported for tests (they pass the zoom, which jsdom doesn't compute).
 */
export function syncFrameMedia(doc: Document, zoom = bodyZoom(doc)): boolean {
	const root = doc.documentElement
	const view = doc.defaultView
	if (!root || !view) return false

	let changed = false
	// ReadiumCSS's cap, read before ours replaces it (fixed layouts have none: left alone)
	let base = baseCaps.get(doc)
	if (base === undefined) {
		base = view.getComputedStyle(root).getPropertyValue(MEDIA_CAP_PROPERTY).trim()
		baseCaps.set(doc, base)
	}
	if (!base) return false

	const cap = compensatedMediaCap(base, zoom)
	const current = root.style.getPropertyValue(MEDIA_CAP_PROPERTY).trim()
	if ((cap ?? '') !== current) {
		if (cap) root.style.setProperty(MEDIA_CAP_PROPERTY, cap, 'important')
		else root.style.removeProperty(MEDIA_CAP_PROPERTY)
		changed = true
	}

	// An image-only page that still doesn't fit its first page: drop the vertical spacing
	// (only scrollable layouts are exempt: they have no first page to fit)
	const scrolled = (root.getAttribute('style') ?? '').includes('readium-scroll-on')
	const imagePage =
		!scrolled &&
		isImageOnlyPage(doc.body) &&
		(root.hasAttribute(IMAGE_PAGE_ATTRIBUTE) || spillsPastFirstColumn(root, doc.body))
	if (imagePage !== root.hasAttribute(IMAGE_PAGE_ATTRIBUTE)) {
		root.toggleAttribute(IMAGE_PAGE_ATTRIBUTE, imagePage)
		changed = true
	}
	return changed
}

/**
 * Keeps a frame's images fitting its page from now on: once when the frame loads, then each
 * time Readium restyles the page (a new text size, layout or theme rewrites `<html>`'s style).
 * `onLayoutChange` runs when the layout changed, so page counts can be measured again.
 */
export function fitFrameMedia(doc: Document, onLayoutChange?: () => void) {
	const root = doc.documentElement
	if (!root || doc.getElementById(FIT_MEDIA_STYLE_ID)) return

	const style = doc.createElement('style')
	style.id = FIT_MEDIA_STYLE_ID
	style.textContent = `:root[${IMAGE_PAGE_ATTRIBUTE}] body, :root[${IMAGE_PAGE_ATTRIBUTE}] body * { margin-top: 0 !important; margin-bottom: 0 !important; padding-top: 0 !important; padding-bottom: 0 !important; }`
	;(doc.head ?? root).appendChild(style)

	const sync = () => {
		try {
			if (syncFrameMedia(doc)) onLayoutChange?.()
		} catch {
			// The frame was torn down
		}
	}
	sync()

	const view = doc.defaultView
	if (!view) return
	const observer = new view.MutationObserver(sync)
	observer.observe(root, { attributes: true, attributeFilter: ['style'] })
	// images without a size load after the page: measure again once they're in
	view.addEventListener('load', sync, { once: true })
}
