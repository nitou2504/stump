import { epubTypesOf, rolesOf } from './noteContent'
import type { LinkFacts } from './resolveLink'

/** A link tapped or clicked in the book's page */
export type FrameLink = {
	/** The link's absolute URL (Readium gives each page a `<base>` with its resource URL) */
	href: string
	/** Its `id`, so the note's link back to it can be left out of the popup */
	id?: string
	facts: LinkFacts
	/** The page (a same-origin frame document) the link is on */
	document: Document
}

/** A pointer that moved further than this between down and up was a swipe or a drag */
export const TAP_SLOP_PX = 10
/** A touch held longer than this is a long press (text selection, the link menu) */
export const LONG_PRESS_MS = 500

/** The link a pointer event happened on, if any */
export function linkFromEventTarget(target: EventTarget | null): Element | null {
	const element = target as Element | null
	if (!element || typeof element.closest !== 'function') return null
	const anchor = element.closest('a[href]')
	const href = anchor?.getAttribute('href')?.trim()
	return anchor && href && !/^javascript:/i.test(href) ? anchor : null
}

function isSuperscript(anchor: Element): boolean {
	if (anchor.closest('sup') || anchor.querySelector('sup')) return true
	try {
		const view = anchor.ownerDocument.defaultView
		const align = view?.getComputedStyle(anchor).verticalAlign
		return align === 'super' || align === 'text-top'
	} catch {
		return false
	}
}

/** What a link says about itself (see {@link LinkFacts}) */
export function readLinkFacts(anchor: Element): LinkFacts {
	return {
		text: (anchor.textContent ?? '').replace(/\s+/g, ' ').trim(),
		epubTypes: epubTypesOf(anchor),
		roles: rolesOf(anchor),
		superscript: isSuperscript(anchor),
	}
}

function toFrameLink(anchor: Element, doc: Document): FrameLink {
	const href = (anchor as HTMLAnchorElement).href || anchor.getAttribute('href') || ''
	return {
		href: typeof href === 'string' ? href : String(href),
		id: anchor.getAttribute('id') ?? undefined,
		facts: readLinkFacts(anchor),
		document: doc,
	}
}

type PointerLike = Pick<PointerEvent, 'clientX' | 'clientY' | 'timeStamp' | 'button'> & {
	pointerType?: string
	isPrimary?: boolean
}

/**
 * Takes over link taps and clicks in a book's page (a same-origin Readium frame) and
 * reports them with `onActivate`, before Readium sees them: Readium resolves links with a
 * path join that breaks absolute URLs (`http:/host/...`) and only knows the pages in the
 * reading order, so footnotes on a `linear="no"` page went nowhere, and on phones the tap
 * could also turn the page. Runs in the capture phase of the frame's document, ahead of
 * Readium's own listeners, and stops the tap there.
 *
 * Left alone, so Readium still handles them: text selections (the annotation toolbar),
 * swipes and drags. A long press on a link is swallowed (no navigation, no page turn).
 * Returns a function that detaches the listeners.
 */
export function attachFrameLinkHandling(
	doc: Document,
	onActivate: (link: FrameLink) => void,
): () => void {
	let down: { x: number; y: number; time: number } | null = null

	const onPointerDown = (event: Event) => {
		const pointer = event as unknown as PointerLike
		if (pointer.isPrimary === false) return
		down = { x: pointer.clientX, y: pointer.clientY, time: event.timeStamp }
	}

	const onPointerUp = (event: Event) => {
		const pointer = event as unknown as PointerLike
		const start = down
		down = null
		if (pointer.isPrimary === false) return

		const anchor = linkFromEventTarget(event.target)
		if (!anchor) return

		const selection = doc.getSelection()
		if (selection && !selection.isCollapsed) return
		if (!start || Math.hypot(pointer.clientX - start.x, pointer.clientY - start.y) > TAP_SLOP_PX) {
			return
		}

		// The tap belongs to the link: Readium must not also handle it
		event.stopPropagation()
		event.preventDefault()

		const isTouch = pointer.pointerType === 'touch' || pointer.pointerType === 'pen'
		if (pointer.button > 0 || (isTouch && event.timeStamp - start.time > LONG_PRESS_MS)) return

		onActivate(toFrameLink(anchor, doc))
	}

	const onClick = (event: Event) => {
		const anchor = linkFromEventTarget(event.target)
		if (!anchor) return
		// The page itself never navigates (it is a blob document)
		event.preventDefault()
		// Enter on a focused link (or a script's click()) clicks it without any pointer event.
		// Stopped here, Readium doesn't also turn it into a tap
		if ((event as MouseEvent).detail === 0) {
			event.stopPropagation()
			onActivate(toFrameLink(anchor, doc))
		}
	}

	doc.addEventListener('pointerdown', onPointerDown, true)
	doc.addEventListener('pointerup', onPointerUp, true)
	doc.addEventListener('click', onClick, true)
	return () => {
		doc.removeEventListener('pointerdown', onPointerDown, true)
		doc.removeEventListener('pointerup', onPointerUp, true)
		doc.removeEventListener('click', onClick, true)
	}
}
