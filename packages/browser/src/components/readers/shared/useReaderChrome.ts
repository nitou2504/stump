import { useCallback, useEffect, useRef, useState } from 'react'

/**
 * Fired on the host window for a mouse move inside a reader's frame (an EPUB page), with the
 * position in the host's coordinates, so the reader chrome can tell the pointer is near the
 * top even when the frame swallows the real `mousemove`
 */
export const READER_POINTER_MOVE_EVENT = 'stump:reader-pointer-move'

export type ReaderPointerMoveDetail = { x: number; y: number }

/** How close to the top edge the mouse must come to reveal the top bar */
export const REVEAL_ZONE_PX = 72
/** Hidden this long after the mouse leaves the bar (or after it was revealed) */
const HIDE_AFTER_MS = 2000
/** Taps (phones) keep it a bit longer, there is no hover to hold it */
const TOUCH_HIDE_AFTER_MS = 4000
/** Shown when the book opens, so it's clear where the controls are */
const INITIAL_SHOW_MS = 2500

/**
 * Re-dispatches mouse moves made inside a same-origin frame on the host window (see
 * {@link READER_POINTER_MOVE_EVENT}). Returns a function that stops forwarding.
 */
export function forwardFramePointerMoves(frameDocument: Document): () => void {
	let pending = false
	let last: MouseEvent | null = null
	const forward = (event: MouseEvent) => {
		last = event
		if (pending) return
		pending = true
		window.requestAnimationFrame(() => {
			pending = false
			const frame = frameDocument.defaultView?.frameElement
			if (!last || !frame) return
			const box = frame.getBoundingClientRect()
			window.dispatchEvent(
				new CustomEvent<ReaderPointerMoveDetail>(READER_POINTER_MOVE_EVENT, {
					detail: { x: box.left + last.clientX, y: box.top + last.clientY },
				}),
			)
		})
	}
	frameDocument.addEventListener('mousemove', forward)
	return () => frameDocument.removeEventListener('mousemove', forward)
}

type Options = {
	/** Keep the bar shown, e.g. while one of its dialogs or menus is open */
	pinned?: boolean
}

export type ReaderChrome = {
	visible: boolean
	/** Show the bar; it hides again by itself after a while */
	show: (forMs?: number) => void
	hide: () => void
	/** A tap in the middle of the page (phones): show or hide */
	toggle: () => void
	/** Spread on the bar: hovering or focusing it keeps it shown */
	barProps: {
		onMouseEnter: () => void
		onMouseLeave: () => void
		onFocusCapture: () => void
		onBlurCapture: () => void
	}
}

/**
 * The visibility of a reader's top bar, which lies over the pages instead of taking room
 * from them: shown when the book opens, when the mouse comes near the top (also from inside
 * an EPUB's frame), on a tap in the middle of the page (phones) or when it gets the keyboard
 * focus; hidden again after a moment unless the pointer is on it or it's pinned.
 */
export function useReaderChrome({ pinned = false }: Options = {}): ReaderChrome {
	const [visible, setVisible] = useState(true)
	const timerRef = useRef<number | undefined>(undefined)
	const holdRef = useRef(false)
	const pinnedRef = useRef(pinned)
	const visibleRef = useRef(visible)
	useEffect(() => {
		visibleRef.current = visible
	}, [visible])

	const clearTimer = () => {
		if (timerRef.current !== undefined) window.clearTimeout(timerRef.current)
		timerRef.current = undefined
	}

	const scheduleHide = useCallback((ms: number) => {
		clearTimer()
		timerRef.current = window.setTimeout(() => {
			timerRef.current = undefined
			if (!holdRef.current && !pinnedRef.current) setVisible(false)
		}, ms)
	}, [])

	const show = useCallback(
		(forMs = HIDE_AFTER_MS) => {
			setVisible(true)
			scheduleHide(forMs)
		},
		[scheduleHide],
	)

	const hide = useCallback(() => {
		clearTimer()
		holdRef.current = false
		setVisible(false)
	}, [])

	const toggle = useCallback(() => {
		if (visibleRef.current) hide()
		else show(TOUCH_HIDE_AFTER_MS)
	}, [hide, show])

	// Shown when the book opens, then out of the way
	useEffect(() => {
		scheduleHide(INITIAL_SHOW_MS)
		return clearTimer
	}, [scheduleHide])

	useEffect(() => {
		pinnedRef.current = pinned
		if (pinned) {
			clearTimer()
			setVisible(true)
		} else if (visibleRef.current) {
			scheduleHide(HIDE_AFTER_MS)
		}
	}, [pinned, scheduleHide])

	// The mouse near the top reveals the bar (a touch's synthetic mouse moves don't count)
	useEffect(() => {
		const reveal = (y: number) => {
			if (y <= REVEAL_ZONE_PX) show()
		}
		const onMouseMove = (event: MouseEvent) => {
			if ((event as PointerEvent).pointerType === 'touch') return
			if (window.matchMedia?.('(pointer: coarse)').matches) return
			reveal(event.clientY)
		}
		const onFrameMove = (event: Event) => {
			if (window.matchMedia?.('(pointer: coarse)').matches) return
			reveal((event as CustomEvent<ReaderPointerMoveDetail>).detail.y)
		}
		window.addEventListener('mousemove', onMouseMove, { passive: true })
		window.addEventListener(READER_POINTER_MOVE_EVENT, onFrameMove)
		return () => {
			window.removeEventListener('mousemove', onMouseMove)
			window.removeEventListener(READER_POINTER_MOVE_EVENT, onFrameMove)
		}
	}, [show])

	const barProps = {
		onMouseEnter: () => {
			holdRef.current = true
			clearTimer()
			setVisible(true)
		},
		onMouseLeave: () => {
			holdRef.current = false
			scheduleHide(HIDE_AFTER_MS)
		},
		onFocusCapture: () => {
			holdRef.current = true
			clearTimer()
			setVisible(true)
		},
		onBlurCapture: () => {
			holdRef.current = false
			scheduleHide(HIDE_AFTER_MS)
		},
	}

	return { visible, show, hide, toggle, barProps }
}

/**
 * Full screen for a reader. The whole document goes full screen (not just the reader), so
 * the dialogs and menus, which are portalled to the end of the page, still show.
 */
export function useDocumentFullscreen() {
	const [isFullscreen, setIsFullscreen] = useState(
		() => typeof document !== 'undefined' && !!document.fullscreenElement,
	)
	useEffect(() => {
		const onChange = () => setIsFullscreen(!!document.fullscreenElement)
		document.addEventListener('fullscreenchange', onChange)
		return () => document.removeEventListener('fullscreenchange', onChange)
	}, [])
	const supported = typeof document !== 'undefined' && !!document.fullscreenEnabled
	const toggle = useCallback(() => {
		if (document.fullscreenElement) {
			void document.exitFullscreen?.()
		} else {
			void document.documentElement.requestFullscreen?.().catch(() => {})
		}
	}, [])
	return { isFullscreen, supported, toggle }
}
