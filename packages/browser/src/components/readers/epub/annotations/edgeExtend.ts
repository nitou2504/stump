import { useCallback, useEffect, useRef, useState } from 'react'

/**
 * Selecting past the end of a page, like KOReader (paginated EPUBs). There is no button: when
 * the selection's moving end rests at the end of what's shown for a moment, the view moves on
 * by one page while the selection keeps growing from where it started.
 *
 * The pages of a chapter are the columns of one document, scrolled sideways (Readium's
 * paginator), so the selection stays one native selection across the move:
 *
 * - two pages side by side: the view moves by one page, half the spread. The page with the
 *   selection's start moves to the left, the next page comes in on the right: the same idea
 *   as KOReader's half-page shift, with whole pages.
 * - one page: a half-page sideways shift would cut every line in two (the next page is to the
 *   right, not below), so the view moves by the whole page and a strip at the top keeps the end
 *   of what's already selected in sight (see `peek`).
 *
 * When the selection is done or dropped the view snaps to a page boundary again: the page (or
 * spread) where the selection ends, or the starting one if it ends there. A selection can't
 * run into the next chapter (another document): at the chapter's end `onChapterEnd` is called.
 */

/** How long the selection's end rests at the end of the page before the view moves on */
export const EDGE_HOLD_MS = 600
/** How long the move takes */
const SHIFT_MS = 220

export type CaretBox = { left: number; right: number; top: number; bottom: number }

const toBox = (rect: DOMRect | null | undefined): CaretBox | null =>
	rect && (rect.width || rect.height)
		? { left: rect.left, right: rect.right, top: rect.top, bottom: rect.bottom }
		: null

/** The caret at the selection's moving end (its focus), in the frame's viewport */
export function focusCaretBox(selection: Selection): CaretBox | null {
	const node = selection.focusNode
	const document = node?.ownerDocument
	if (!node || !document) return null
	try {
		const range = document.createRange()
		if (node.nodeType === Node.TEXT_NODE) {
			const length = node.textContent?.length ?? 0
			if (!length) return null
			// the selection's last letter (or the first, at the very start of the text)
			const offset = Math.min(Math.max(1, selection.focusOffset), length)
			range.setStart(node, offset - 1)
			range.setEnd(node, offset)
		} else {
			range.selectNodeContents(node)
		}
		const rects = range.getClientRects()
		return toBox(rects[rects.length - 1] ?? range.getBoundingClientRect())
	} catch {
		return null
	}
}

/**
 * Where reading continues after the caret's line: the first letter after the selection's
 * end that's not on its line (the next line, or the top of the next column). `null` at the
 * end of the chapter.
 */
export function nextLineBox(
	selection: Selection,
	caret: CaretBox,
	maxLetters = 1200,
): CaretBox | null {
	const focus = selection.focusNode
	const document = focus?.ownerDocument
	if (!focus || !document?.body) return null
	const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT)
	let node: Node | null
	let offset = 0
	if (focus.nodeType === Node.TEXT_NODE) {
		walker.currentNode = focus
		node = focus
		offset = selection.focusOffset
	} else {
		walker.currentNode = focus
		node = walker.nextNode()
	}
	const range = document.createRange()
	let checked = 0
	while (node && checked < maxLetters) {
		const text = node.textContent ?? ''
		for (let index = offset; index < text.length && checked < maxLetters; index++) {
			checked++
			if (/\s/.test(text[index]!)) continue
			range.setStart(node, index)
			range.setEnd(node, index + 1)
			const box = toBox(range.getClientRects()[0])
			if (!box) continue
			const sameLine = box.top < caret.bottom - 1 && box.bottom > caret.top + 1
			if (!sameLine) return box
		}
		node = walker.nextNode()
		offset = 0
	}
	return null
}

/**
 * Whether the selection's end is at the end of what's shown: on the last visible line (the
 * next line starts past the edge, on a page that isn't shown), or already past the edge
 */
export function isAtVisibleEnd(
	caret: CaretBox,
	next: CaretBox | null,
	viewportWidth: number,
	rtl = false,
): boolean {
	const past = (box: CaretBox) => (rtl ? box.right <= 1 : box.left >= viewportWidth - 1)
	if (past(caret)) return true
	return !!next && past(next)
}

/** The pages (columns) shown at once: 2 for a spread */
export function columnsPerScreen(frameWindow: Window): number {
	const count = parseInt(
		frameWindow
			.getComputedStyle(frameWindow.document.documentElement)
			.getPropertyValue('column-count'),
	)
	return Number.isFinite(count) && count > 0 ? count : 1
}

/**
 * Where the view goes when the selection is done: the start of the spread holding the
 * column (page) where it ends, as a scroll distance from the chapter's start
 */
export function spreadStartFor(column: number, columns: number, viewportWidth: number): number {
	return Math.floor(Math.max(0, column) / Math.max(1, columns)) * viewportWidth
}

const scrollerOf = (document: Document) => document.scrollingElement as HTMLElement | null

const isRtl = (document: Document) =>
	(document.documentElement.dir || document.body?.dir || '').toLowerCase() === 'rtl'

/** How far the view is from the chapter's start, whatever the reading direction */
const distanceOf = (document: Document) => Math.abs(scrollerOf(document)?.scrollLeft ?? 0)

/** Moves the view (RTL documents scroll to negative offsets) */
function scrollViewTo(document: Document, distance: number, animate: boolean) {
	const scroller = scrollerOf(document)
	if (!scroller) return
	const sign = isRtl(document) ? -1 : 1
	const from = distanceOf(document)
	if (!animate || from === distance) {
		scroller.scrollLeft = sign * distance
		return
	}
	const view = document.defaultView ?? window
	const started = view.performance.now()
	const step = (now: number) => {
		const t = Math.min(1, (now - started) / SHIFT_MS)
		const eased = t < 0.5 ? 2 * t * t : 1 - (-2 * t + 2) ** 2 / 2
		scroller.scrollLeft = sign * (from + (distance - from) * eased)
		if (t < 1) view.requestAnimationFrame(step)
	}
	view.requestAnimationFrame(step)
}

type Shift = {
	document: Document
	/** Where the view was (distance from the chapter's start) before the first move */
	origin: number
	columns: number
	/** The end of what was already selected, shown in the strip (one page) */
	peek: string
	/** Where the selection's end was put after the move (one page, touch), to tell if it moved */
	placedFocus: { node: Node; offset: number } | null
}

type Options = {
	containerRef: React.RefObject<HTMLElement | null>
	/** Paginated and reflowable: not in continuous scroll or a fixed-layout book */
	enabled: boolean
	/** Touch screen: the selection's end is put on the new page so its handle can be dragged */
	isTouch: boolean
	/** The view was set to a page boundary by hand: Readium should take it from there */
	onSettled: () => void
	/** The selection reached the end of the chapter */
	onChapterEnd: () => void
}

/** Keeps the selection going past the end of the page; see the module's comment */
export function useEdgeHoldExtend({
	containerRef,
	enabled,
	isTouch,
	onSettled,
	onChapterEnd,
}: Options) {
	const [peek, setPeek] = useState<string | null>(null)
	const shiftRef = useRef<Shift | null>(null)
	/** The column (page, from the chapter's start) where the selection's end was last seen */
	const lastColumnRef = useRef<number | null>(null)
	const holdRef = useRef<number | undefined>(undefined)
	const mouseDownRef = useRef(false)
	const callbacksRef = useRef({ onSettled, onChapterEnd, isTouch })
	useEffect(() => {
		callbacksRef.current = { onSettled, onChapterEnd, isTouch }
	}, [onSettled, onChapterEnd, isTouch])

	/** Back to a page boundary: the spread where the selection ended, or where it started */
	const settle = useCallback(() => {
		const shift = shiftRef.current
		shiftRef.current = null
		setPeek(null)
		if (!shift) return
		const view = shift.document.defaultView
		if (!view) return
		const width = view.innerWidth
		const column = lastColumnRef.current
		const target = column == null ? shift.origin : spreadStartFor(column, shift.columns, width)
		scrollViewTo(shift.document, target, false)
		callbacksRef.current.onSettled()
	}, [])

	const shiftForward = useCallback((document: Document) => {
		const view = document.defaultView
		const scroller = scrollerOf(document)
		const selection = document.getSelection()
		if (!view || !scroller || !selection || selection.isCollapsed) return
		const width = view.innerWidth
		const columns = columnsPerScreen(view)
		const step = width / columns
		const max = scroller.scrollWidth - width
		const from = distanceOf(document)
		if (from >= max - 1) {
			callbacksRef.current.onChapterEnd()
			return
		}
		const to = Math.min(max, from + step)
		const selected = selection.toString()
		shiftRef.current = {
			document,
			origin: shiftRef.current?.origin ?? from,
			columns,
			peek: selected,
			placedFocus: null,
		}
		scrollViewTo(document, to, true)
		if (columns === 1) {
			setPeek(selected.length > 140 ? `…${selected.slice(-140).trimStart()}` : selected)
			// On a touch screen the end's handle would be on the page that just left: put the
			// selection's end at the top of the new page, where it can be dragged on
			if (callbacksRef.current.isTouch) {
				window.setTimeout(() => {
					const placed = placeFocusAtPageStart(document, selection, width)
					if (shiftRef.current && placed) shiftRef.current.placedFocus = placed
				}, SHIFT_MS + 30)
			}
		}
	}, [])

	useEffect(() => {
		const container = containerRef.current
		if (!enabled || !container) return undefined
		const attached = new Map<Document, () => void>()

		const onSelectionChange = (document: Document) => {
			window.clearTimeout(holdRef.current)
			const selection = document.getSelection()
			const view = document.defaultView
			if (!selection || !view || selection.isCollapsed || !selection.rangeCount) {
				// done (an annotation was made, copied, dismissed): snap to a page boundary
				if (shiftRef.current?.document === document) {
					const shift = shiftRef.current
					// the end was put on the new page and never moved: it ended on the first page
					if (shift.placedFocus) lastColumnRef.current = null
					settle()
				}
				lastColumnRef.current = null
				return
			}
			const caret = focusCaretBox(selection)
			if (!caret) return
			const width = view.innerWidth
			const columns = columnsPerScreen(view)
			const shift = shiftRef.current
			const movedSincePlaced =
				!shift?.placedFocus ||
				shift.placedFocus.node !== selection.focusNode ||
				shift.placedFocus.offset !== selection.focusOffset
			if (movedSincePlaced) {
				if (shift) shift.placedFocus = null
				const left = isRtl(document) ? width - caret.right : caret.left
				lastColumnRef.current = Math.floor(
					(distanceOf(document) + Math.max(0, left)) / (width / columns),
				)
			}
			const atEnd = isAtVisibleEnd(caret, nextLineBox(selection, caret), width, isRtl(document))
			// with a mouse, only while the button is still down (dragging); on touch the
			// handles give no events, so resting there is enough
			if (!atEnd || (!callbacksRef.current.isTouch && !mouseDownRef.current)) return
			holdRef.current = window.setTimeout(() => {
				const current = document.getSelection()
				if (!current || current.isCollapsed) return
				const box = focusCaretBox(current)
				if (!box) return
				const still = isAtVisibleEnd(
					box,
					nextLineBox(current, box),
					view.innerWidth,
					isRtl(document),
				)
				if (still && (callbacksRef.current.isTouch || mouseDownRef.current)) shiftForward(document)
			}, EDGE_HOLD_MS)
		}

		const attach = () => {
			for (const frame of container.querySelectorAll('iframe')) {
				let document: Document | null = null
				try {
					document = frame.contentDocument
				} catch {
					document = null
				}
				if (!document || attached.has(document)) continue
				const doc = document
				const selectionListener = () => onSelectionChange(doc)
				const down = (event: MouseEvent) => {
					if (event.button === 0) mouseDownRef.current = true
				}
				const up = () => {
					mouseDownRef.current = false
					window.clearTimeout(holdRef.current)
				}
				doc.addEventListener('selectionchange', selectionListener)
				doc.addEventListener('mousedown', down)
				doc.addEventListener('mouseup', up)
				attached.set(doc, () => {
					doc.removeEventListener('selectionchange', selectionListener)
					doc.removeEventListener('mousedown', down)
					doc.removeEventListener('mouseup', up)
				})
			}
		}
		attach()
		// Readium adds and replaces frames as the chapters load
		const observer = new MutationObserver(attach)
		observer.observe(container, { childList: true, subtree: true })
		// frames load their documents after they're added
		const interval = window.setInterval(attach, 1500)
		const onWindowMouseUp = () => {
			mouseDownRef.current = false
		}
		window.addEventListener('mouseup', onWindowMouseUp)

		return () => {
			observer.disconnect()
			window.clearInterval(interval)
			window.clearTimeout(holdRef.current)
			window.removeEventListener('mouseup', onWindowMouseUp)
			for (const detach of attached.values()) detach()
		}
	}, [enabled, containerRef, settle, shiftForward])

	return { peek, settle }
}

/**
 * Moves the selection's end to the first letter shown on the page (after a whole-page move),
 * keeping its start; returns where it was put
 */
function placeFocusAtPageStart(
	document: Document,
	selection: Selection,
	viewportWidth: number,
): { node: Node; offset: number } | null {
	if (!selection.focusNode || !document.body) return null
	const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT)
	walker.currentNode = selection.focusNode
	let node: Node | null =
		selection.focusNode.nodeType === Node.TEXT_NODE ? selection.focusNode : walker.nextNode()
	let offset = selection.focusNode.nodeType === Node.TEXT_NODE ? selection.focusOffset : 0
	const range = document.createRange()
	let checked = 0
	while (node && checked < 4000) {
		const text = node.textContent ?? ''
		for (let index = offset; index < text.length; index++) {
			checked++
			if (/\s/.test(text[index]!)) continue
			range.setStart(node, index)
			range.setEnd(node, index + 1)
			const box = range.getClientRects()[0]
			if (box && box.left >= 0 && box.right <= viewportWidth) {
				// the first word on the page, so the end's handle sits right after it
				const space = text.slice(index).search(/\s/)
				const end = space === -1 ? text.length : index + space
				try {
					selection.extend(node, end)
					return { node, offset: end }
				} catch {
					return null
				}
			}
		}
		node = walker.nextNode()
		offset = 0
	}
	return null
}
