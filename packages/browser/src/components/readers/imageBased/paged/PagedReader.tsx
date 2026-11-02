import { ReadingDirection, ReadingImageScaleFit } from '@stump/graphql'
import { memo, useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import { useHotkeys } from 'react-hotkeys-hook'

import { EntityImage } from '@/components/entity'
import { useBookPreferences } from '@/scenes/book/reader/useBookPreferences'

import {
	edgeSwipeTurn,
	isCoarsePointer,
	pageTapZone,
	type ScrollEdges,
	scrollEdges,
	SWIPE_SLOP,
	swipeOf,
	swipeTurn,
	zoneTurn,
} from '../../shared/gestures'
import { panOrTurn } from '../../shared/panKeys'
import { cancelScrollMove, scrollMoveTo, scrollTarget } from '../../shared/scrollMove'
import {
	clampRatio,
	distance,
	DOUBLE_TAP_MS,
	isDoubleTap,
	midpoint,
	pinchTransform,
	type Point,
} from '../../shared/zoomAnchor'
import { useImageBaseReaderContext } from '../context'
import { analysePage, usePageAnalyses } from '../pageAnalysis'
import {
	type Box,
	buildViews,
	clampZoom,
	entryPos,
	fitSize,
	isWide,
	itemBox,
	MAX_ZOOM,
	MIN_ZOOM,
	type Size,
	stepView,
	type ViewItem,
	viewOfPage,
	type ViewPos,
	wideHandling,
	wideZoomSize,
} from './pagedView'

export type PagedReaderProps = {
	/** The current page which the reader should render (1-based) */
	currentPage: number
	/** A callback that is called in order to change the page */
	onPageChange: (page: number) => void
}

/** Taps right after a pinch, a pan or a step's scroll are the end of it, not a tap */
const GESTURE_QUIET_MS = 250
/** A press longer than this isn't a tap */
const TAP_MAX_MS = 500
/** A wheel pause longer than this starts a new wheel gesture */
const WHEEL_GESTURE_GAP_MS = 300
/** Wheel distance (px) that turns a page that fits the screen */
const WHEEL_TURN_DISTANCE = 80

/**
 * The comic reader, one page (or one spread) at a time, like Mihon or KOReader:
 *
 * - The page is drawn at its fit inside a scroller as big as the screen. It never moves under
 *   the finger at its fit; a page bigger than the screen (fit width, zoomed, a wide page) is
 *   panned by the browser's own scrolling, only along the sides that overflow, and never shows
 *   empty space.
 * - Taps at the sides, the arrow keys and Space step through a page bigger than the screen
 *   (the next part, with a small overlap, in reading order) and turn the page after its last
 *   part. Page Up / Page Down always turn. A quick sideways swipe turns the page; on a page
 *   bigger than the screen a finger pans it, and a swipe turns only from the edge it was
 *   already at (a swipe that reaches the edge stops there, the next one turns).
 * - A pinch (or Ctrl + wheel, or a double tap in the middle) zooms around the fingers, never
 *   out past the fit.
 * - Pages can be cropped to their content, and with Fit Page wide pages on a narrow screen
 *   zoomed so half of them fills the width, or split in two (any other fit draws them at that
 *   fit, like every page).
 */
function PagedReader({ currentPage, onPageChange }: PagedReaderProps) {
	const { pageSets, book, getPageUrl, imageSizes, setPageSize } = useImageBaseReaderContext()
	const {
		bookPreferences: {
			tapSidesToNavigate,
			imageScaling,
			readingDirection,
			brightness,
			cropBorders,
			widePages = 'zoom',
		},
		settings: { showToolBar },
		setSettings,
	} = useBookPreferences({ book })

	const rtl = readingDirection === ReadingDirection.Rtl
	const fit = imageScaling?.scaleToFit ?? ReadingImageScaleFit.Height
	const analyses = usePageAnalyses(book.id)

	const scrollRef = useRef<HTMLDivElement | null>(null)
	const contentRef = useRef<HTMLDivElement | null>(null)

	// The screen the pages are drawn in (the scroller, inside the safe area)
	const [viewport, setViewport] = useState<Size>({ w: 0, h: 0 })
	useLayoutEffect(() => {
		const element = scrollRef.current
		if (!element) return undefined
		const measure = () => setViewport({ w: element.clientWidth, h: element.clientHeight })
		measure()
		const observer = new ResizeObserver(measure)
		observer.observe(element)
		return () => observer.disconnect()
	}, [])

	const portrait = viewport.h > viewport.w
	const onePageAtATime = pageSets.every((set) => set.length === 1)
	const ratioOf = useCallback(
		(page: number) => {
			const size = imageSizes[page]
			return size ? size.width / size.height : undefined
		},
		[imageSizes],
	)
	// the wide-page setting is Fit Page's: with another fit a wide page shows at that fit
	const wide = wideHandling({ fit, widePages, portrait, onePageAtATime })
	const split = wide === 'split'
	const views = useMemo(() => buildViews(pageSets, ratioOf, split), [pageSets, ratioOf, split])

	// Zoom (1 = the fit), reset on every turn
	const [zoom, setZoom] = useState(1)
	// Where to put the screen after the next layout: the entry of a turned-to view, or the spot
	// a zoom keeps under the fingers
	const pendingEntryRef = useRef<{ toward: 1 | -1; forward: boolean } | null>({
		toward: rtl ? -1 : 1,
		forward: true,
	})
	const pendingAnchorRef = useRef<{ fx: number; fy: number; vx: number; vy: number } | null>(null)
	/**
	 * The side the view on screen was entered from, until the reader moves it: a size that
	 * arrives later (the page's crop, its image) keeps the view at that side, its reading start
	 */
	const enteredRef = useRef<{ toward: 1 | -1; forward: boolean } | null>(null)
	const lastGestureRef = useRef(0)

	// The view on screen: follows the page in the URL (go to page, progress), keeping the half
	// of a split page that a step or turn chose
	// a split page opens on the half read first: the right one in a right-to-left book
	const firstHalf = rtl ? 'right' : 'left'
	const [viewIndex, setViewIndex] = useState(() =>
		Math.max(0, viewOfPage(views, currentPage - 1, firstHalf)),
	)
	/** A view a step or turn chose, until the URL catches up with its page */
	const chosenViewRef = useRef<{ index: number; page: number } | null>(null)
	// the item last shown, to tell a page that got split from a half chosen by a step
	const shownItemRef = useRef<ViewItem | null>(null)
	useEffect(() => {
		const chosen = chosenViewRef.current
		if (chosen) {
			// the URL hasn't caught up with a turn yet (e.g. a page's size arrived meanwhile)
			if (chosen.page !== currentPage - 1) return
			chosenViewRef.current = null
			if (views[chosen.index]?.some((item) => item.page === chosen.page)) {
				setViewIndex(chosen.index)
				return
			}
		}
		// read now: the updater below runs at the next render, after `shownItemRef` moved on
		const before = shownItemRef.current
		setViewIndex((index) => {
			const item = views[index]?.find((one) => one.page === currentPage - 1)
			if (item) {
				// a page on screen that has just been split (its size became known): its first half
				const justSplit =
					item.part !== 'whole' && before?.page === item.page && before.part === 'whole'
				return justSplit ? Math.max(0, viewOfPage(views, currentPage - 1, firstHalf)) : index
			}
			// another page from outside (go to page, the pages grid): its start, at its fit
			pendingEntryRef.current = { toward: rtl ? -1 : 1, forward: true }
			setZoom(1)
			return Math.max(0, viewOfPage(views, currentPage - 1, firstHalf))
		})
	}, [currentPage, views, rtl, firstHalf])

	const items = useMemo<ViewItem[]>(
		() => views[viewIndex] ?? [{ page: currentPage - 1, part: 'whole' }],
		[views, viewIndex, currentPage],
	)
	useEffect(() => {
		shownItemRef.current = items.length === 1 ? items[0]! : null
	}, [items])

	// What's drawn: each item's box in its image (cropped, a half), and the view's size at its fit
	const crops = cropBorders && items.length === 1
	const geometry = useMemo(() => {
		const sizes = items.map((item) => imageSizes[item.page])
		if (sizes.some((size) => !size) || !viewport.w || !viewport.h) return null
		const boxes = items.map((item, index) => {
			const size = sizes[index]!
			const analysis = crops ? analyses[item.page + 1] : undefined
			return itemBox(item.part, { w: size.width, h: size.height }, analysis?.crop)
		})
		const only = items.length === 1 ? items[0] : undefined
		const onlyBox = boxes[0]!
		const wideZoom =
			!!only && only.part === 'whole' && wide === 'zoom' && isWide(onlyBox.w / onlyBox.h)
		const base = wideZoom
			? wideZoomSize(onlyBox.w / onlyBox.h, viewport)
			: fitSize(boxes, fit, viewport)
		return { boxes, base }
	}, [items, imageSizes, viewport, crops, analyses, wide, fit])

	const content = useMemo<Size>(
		() =>
			geometry
				? { w: geometry.base.w * zoom, h: geometry.base.h * zoom }
				: { w: viewport.w, h: viewport.h },
		[geometry, zoom, viewport],
	)
	const overflowX = content.w > viewport.w + 1
	const overflowY = content.h > viewport.h + 1

	// A new fit (or wide-page setting, or crop) shows the view from its reading start, at the fit
	const fitKey = `${fit}|${widePages}|${!!cropBorders}`
	const fitKeyRef = useRef(fitKey)

	useLayoutEffect(() => {
		const element = scrollRef.current
		const inner = contentRef.current
		if (!element || !geometry) return
		if (inner) {
			inner.style.transform = ''
			inner.style.willChange = ''
		}
		if (fitKeyRef.current !== fitKey) {
			fitKeyRef.current = fitKey
			pendingAnchorRef.current = null
			// (a turn under way keeps its own entry)
			pendingEntryRef.current ??= { toward: rtl ? -1 : 1, forward: true }
			// laid out again at the fit, then entered (the next run of this effect)
			if (zoom !== 1) {
				setZoom(1)
				return
			}
		}
		const anchor = pendingAnchorRef.current
		if (anchor) {
			pendingAnchorRef.current = null
			enteredRef.current = null
			const offsetX = Math.max(0, (viewport.w - content.w) / 2)
			const offsetY = Math.max(0, (viewport.h - content.h) / 2)
			element.scrollTo({
				left: offsetX + anchor.fx * content.w - anchor.vx,
				top: offsetY + anchor.fy * content.h - anchor.vy,
			})
			return
		}
		const entry = pendingEntryRef.current ?? enteredRef.current
		if (entry) {
			pendingEntryRef.current = null
			enteredRef.current = entry
			const pos = entryPos(content, viewport, entry.toward, entry.forward)
			element.scrollTo({ left: pos.x, top: pos.y })
		}
	}, [viewIndex, geometry, content, viewport, fitKey, rtl, zoom])

	// Changing the view -----------------------------------------------------------------------------

	/** Shows another view, entered from the side the input comes from */
	const turnTo = useCallback(
		(index: number, toward: 1 | -1) => {
			const view = views[index]
			if (!view?.length) return
			const forward = rtl ? toward < 0 : toward > 0
			// a move still under way belongs to the view being left
			if (scrollRef.current) cancelScrollMove(scrollRef.current)
			pendingEntryRef.current = { toward, forward }
			setZoom(1)
			setViewIndex(index)
			// the page reported: the one read first in the new view
			const pages = view.map((item) => item.page)
			const page = (forward ? Math.min(...pages) : Math.max(...pages)) + 1
			if (page !== currentPage) {
				chosenViewRef.current = { index, page: page - 1 }
				onPageChange(page)
			}
		},
		[views, rtl, currentPage, onPageChange],
	)

	/**
	 * Moves the screen to a spot of the view, quickly and smoothly (the shared key move: a
	 * press during a move retargets it, so a held key moves on steadily)
	 */
	const scrollToSmoothly = useCallback((target: ViewPos) => {
		enteredRef.current = null
		const element = scrollRef.current
		if (element) scrollMoveTo(element, target)
	}, [])
	useEffect(() => {
		const element = scrollRef.current
		return () => {
			if (element) cancelScrollMove(element)
		}
	}, [])

	/**
	 * A step on screen: rightward (+1) or leftward (-1). Shows the next part of a view bigger
	 * than the screen, or turns to the next view at its end.
	 */
	const step = useCallback(
		(toward: 1 | -1) => {
			const element = scrollRef.current
			if (!element) return
			const forward = rtl ? toward < 0 : toward > 0
			// from where a move under way is going: presses during a move add up
			const next = stepView({
				pos: scrollTarget(element),
				content: { w: element.scrollWidth, h: element.scrollHeight },
				viewport: { w: element.clientWidth, h: element.clientHeight },
				toward,
				forward,
			})
			if (next === 'turn') turnTo(viewIndex + toward, toward)
			else scrollToSmoothly(next)
		},
		[rtl, turnTo, viewIndex, scrollToSmoothly],
	)

	/** A whole page (view) turn, whatever part of it shows */
	const turn = useCallback(
		(toward: 1 | -1) => turnTo(viewIndex + toward, toward),
		[turnTo, viewIndex],
	)

	/** Reading order: forward is leftward in a right-to-left book */
	const forwardToward: 1 | -1 = rtl ? -1 : 1

	// Keys ------------------------------------------------------------------------------------------
	useHotkeys(
		'right, left, up, down, pageup, pagedown, space, shift+space, escape',
		(event, handler) => {
			// the settings, the pages grid: their own keys
			if (document.querySelector('[role="dialog"][data-state="open"]')) return
			const key = handler.keys?.at(0)
			const element = scrollRef.current
			event.preventDefault()
			switch (key) {
				case 'right':
					step(1)
					break
				case 'left':
					step(-1)
					break
				case 'up':
				case 'down': {
					// fine moves down or up the page, turning at its end (forward going down)
					if (!element) break
					const at = scrollTarget(element)
					const result = panOrTurn(
						{
							scrollLeft: at.x,
							scrollTop: at.y,
							scrollWidth: element.scrollWidth,
							scrollHeight: element.scrollHeight,
							clientWidth: element.clientWidth,
							clientHeight: element.clientHeight,
						},
						{ dx: 0, dy: key === 'down' ? 1 : -1 },
					)
					if (result.type === 'turn')
						turn(key === 'down' ? forwardToward : (-forwardToward as 1 | -1))
					else scrollToSmoothly({ x: result.left, y: result.top })
					break
				}
				case 'pageup':
					turn(-forwardToward as 1 | -1)
					break
				case 'pagedown':
					turn(forwardToward)
					break
				case 'space':
					step(handler.shift ? (-forwardToward as 1 | -1) : forwardToward)
					break
				case 'escape':
					setSettings({ showToolBar: false })
					break
			}
		},
		[step, turn, forwardToward, scrollToSmoothly, setSettings],
	)

	// Zoom ------------------------------------------------------------------------------------------

	/** Commits a zoom, keeping the spot under `point` (screen px) where it is */
	const zoomAt = useCallback(
		(next: number, point: Point, from?: Point) => {
			const element = scrollRef.current
			if (!element) return
			const clamped = clampZoom(next)
			const box = element.getBoundingClientRect()
			const offsetX = Math.max(0, (box.width - content.w) / 2)
			const offsetY = Math.max(0, (box.height - content.h) / 2)
			// the spot of the view under the starting point, as fractions of the view
			const start = from ?? point
			const fx = (element.scrollLeft + start.x - box.left - offsetX) / Math.max(1, content.w)
			const fy = (element.scrollTop + start.y - box.top - offsetY) / Math.max(1, content.h)
			pendingAnchorRef.current = {
				fx: Math.min(1, Math.max(0, fx)),
				fy: Math.min(1, Math.max(0, fy)),
				vx: point.x - box.left,
				vy: point.y - box.top,
			}
			lastGestureRef.current = performance.now()
			if (clamped === zoom) {
				pendingAnchorRef.current = null
				if (contentRef.current) contentRef.current.style.transform = ''
				return
			}
			setZoom(clamped)
		},
		[content.w, content.h, zoom],
	)
	const zoomAtRef = useRef(zoomAt)
	useEffect(() => {
		zoomAtRef.current = zoomAt
	}, [zoomAt])
	const zoomRef = useRef(zoom)
	useEffect(() => {
		zoomRef.current = zoom
	}, [zoom])

	// Pointer: taps, swipes ------------------------------------------------------------------------
	const pressRef = useRef<{
		id: number
		x: number
		y: number
		at: number
		cancelled: boolean
	} | null>(null)
	const lastTapRef = useRef<{ time: number; point: Point } | null>(null)
	const pendingTapRef = useRef<number | undefined>(undefined)
	useEffect(() => () => window.clearTimeout(pendingTapRef.current), [])

	const toggleBar = useCallback(
		() => setSettings({ showToolBar: !showToolBar }),
		[setSettings, showToolBar],
	)

	const onPointerDown = useCallback((event: React.PointerEvent) => {
		// the reader takes over where the view is
		enteredRef.current = null
		if (!event.isPrimary || event.button > 0) return
		pressRef.current = {
			id: event.pointerId,
			x: event.clientX,
			y: event.clientY,
			at: performance.now(),
			cancelled: false,
		}
	}, [])

	const onPointerCancel = useCallback(() => {
		// the browser took the gesture (a scroll or a pinch): not a tap, not a swipe
		if (pressRef.current) pressRef.current.cancelled = true
		lastGestureRef.current = performance.now()
	}, [])

	const onPointerUp = useCallback(
		(event: React.PointerEvent) => {
			const press = pressRef.current
			pressRef.current = null
			const element = scrollRef.current
			if (!press || press.id !== event.pointerId || press.cancelled || !element) return
			if (performance.now() - lastGestureRef.current < GESTURE_QUIET_MS) return
			const dx = event.clientX - press.x
			const dy = event.clientY - press.y
			const ms = performance.now() - press.at
			const box = element.getBoundingClientRect()

			if (Math.hypot(dx, dy) > SWIPE_SLOP) {
				// a finger's swipe is judged from its touches, with the page's edges (see the touch
				// listeners); a mouse drag the page didn't follow: a deliberate sideways one turns the
				// page (moving left shows what's to the right)
				if (event.pointerType === 'touch') return
				const swipe = swipeTurn(swipeOf(dx, dy, ms, box.width))
				if (swipe) turn(swipe)
				return
			}
			if (ms > TAP_MAX_MS) return

			const coarse = event.pointerType !== 'mouse' && isCoarsePointer()
			const zone = pageTapZone((event.clientX - box.left) / box.width, coarse)
			if (zone !== 'middle' && tapSidesToNavigate !== false) {
				window.clearTimeout(pendingTapRef.current)
				lastTapRef.current = null
				step(zoneTurn(zone) as 1 | -1)
				return
			}
			if (!coarse) {
				toggleBar()
				return
			}
			// the middle of a touch screen: a double tap zooms to 2x around it (or back to the fit);
			// a single tap shows or hides the top bar once it's clear no second tap follows
			const point = { x: event.clientX, y: event.clientY }
			const now = performance.now()
			if (isDoubleTap(lastTapRef.current, now, point)) {
				window.clearTimeout(pendingTapRef.current)
				lastTapRef.current = null
				zoomAt(zoom > 1.05 ? 1 : 2, point)
				return
			}
			lastTapRef.current = { time: now, point }
			window.clearTimeout(pendingTapRef.current)
			pendingTapRef.current = window.setTimeout(toggleBar, DOUBLE_TAP_MS)
		},
		[turn, step, tapSidesToNavigate, toggleBar, zoom, zoomAt],
	)

	// The page turn for a finger's swipe (kept current for the touch listeners below)
	const turnRef = useRef(turn)
	useEffect(() => {
		turnRef.current = turn
	}, [turn])

	// Touches: a pinch, and a finger's swipe.
	// - A pinch: only a GPU transform while the fingers move, the zoom committed once they lift.
	//   The transform is held to what the committed zoom can show (the page covers the screen or
	//   stays centred), so at the fit it holds still instead of wandering and springing back.
	// - A swipe: judged when the finger lifts (the page never follows the finger). The browser
	//   pans a page bigger than the screen; a swipe turns the page only when the page was already
	//   at its edge that way when the finger came down (KOReader, Mihon). A page that fits is at
	//   both edges, so any deliberate swipe turns it.
	useEffect(() => {
		const element = scrollRef.current
		if (!element) return undefined
		let start: {
			distance: number
			mid: Point
			origin: Point
			zoom: number
			bounds: { content: DOMRect; view: DOMRect }
		} | null = null
		let ratio = 1
		let mid: Point = { x: 0, y: 0 }
		let swipeStart: { x: number; y: number; time: number; edges: ScrollEdges } | null = null
		const pointOf = (touch: Touch): Point => ({ x: touch.clientX, y: touch.clientY })
		const onTouchStart = (event: TouchEvent) => {
			const first = event.touches[0]
			// a second finger makes it a pinch, not a swipe
			swipeStart =
				event.touches.length === 1 && first
					? {
							x: first.clientX,
							y: first.clientY,
							time: performance.now(),
							edges: scrollEdges(element),
						}
					: null
			if (event.touches.length !== 2) return
			const a = pointOf(event.touches[0]!)
			const b = pointOf(event.touches[1]!)
			mid = midpoint(a, b)
			const inner = contentRef.current
			if (!inner) return
			const innerBox = inner.getBoundingClientRect()
			start = {
				distance: Math.max(1, distance(a, b)),
				mid,
				// the content point under the fingers, in the content's own px
				origin: { x: mid.x - innerBox.left, y: mid.y - innerBox.top },
				zoom: zoomRef.current,
				bounds: { content: innerBox, view: element.getBoundingClientRect() },
			}
			ratio = 1
			inner.style.willChange = 'transform'
			if (pressRef.current) pressRef.current.cancelled = true
		}
		const onTouchMove = (event: TouchEvent) => {
			if (!start || event.touches.length !== 2) return
			event.preventDefault()
			const a = pointOf(event.touches[0]!)
			const b = pointOf(event.touches[1]!)
			mid = midpoint(a, b)
			ratio = clampRatio(start.zoom, distance(a, b) / start.distance, MIN_ZOOM, MAX_ZOOM)
			const inner = contentRef.current
			if (inner) {
				const look = pinchTransform(start.origin, start.mid, mid, ratio, start.bounds)
				inner.style.transformOrigin = look.transformOrigin
				inner.style.transform = look.transform
			}
		}
		const endPinch = () => {
			if (!start) return
			const { zoom: startZoom, mid: startMid } = start
			start = null
			lastGestureRef.current = performance.now()
			// measured without the gesture's transform, so the anchor is where the page really is
			const inner = contentRef.current
			const transform = inner?.style.transform ?? ''
			if (inner) inner.style.transform = ''
			const next = clampZoom(startZoom * ratio)
			zoomAtRef.current(next, mid, startMid)
			// kept until the new zoom is laid out (in the same frame), so nothing jumps
			if (inner && transform && next !== startZoom) inner.style.transform = transform
			ratio = 1
		}
		const onTouchEnd = (event: TouchEvent) => {
			if (event.touches.length > 0) return
			if (start) {
				endPinch()
				return
			}
			const from = swipeStart
			swipeStart = null
			const touch = event.changedTouches[0]
			if (!from || !touch) return
			const swipe = swipeOf(
				touch.clientX - from.x,
				touch.clientY - from.y,
				performance.now() - from.time,
				element.clientWidth,
			)
			const screenTurn = edgeSwipeTurn(swipe, from.edges, scrollEdges(element))
			if (!screenTurn) return
			lastGestureRef.current = performance.now()
			turnRef.current(screenTurn)
		}
		const onTouchCancel = () => {
			swipeStart = null
			endPinch()
		}
		element.addEventListener('touchstart', onTouchStart, { passive: true })
		element.addEventListener('touchmove', onTouchMove, { passive: false })
		element.addEventListener('touchend', onTouchEnd)
		element.addEventListener('touchcancel', onTouchCancel)
		return () => {
			element.removeEventListener('touchstart', onTouchStart)
			element.removeEventListener('touchmove', onTouchMove)
			element.removeEventListener('touchend', onTouchEnd)
			element.removeEventListener('touchcancel', onTouchCancel)
		}
	}, [])

	// The wheel: Ctrl/⌘ + wheel (a trackpad pinch) zooms around the mouse; otherwise it scrolls a
	// page bigger than the screen, and turns the page at its end on a new wheel gesture (or right
	// away when the page fits)
	const forwardRef = useRef(forwardToward)
	useEffect(() => {
		forwardRef.current = forwardToward
	}, [forwardToward])
	useEffect(() => {
		const element = scrollRef.current
		if (!element) return undefined
		let zoomRatio = 1
		let zoomPoint: Point = { x: 0, y: 0 }
		let zoomOrigin: Point | null = null
		let zoomBounds: { content: DOMRect; view: DOMRect } | null = null
		let zoomTimeout: number | undefined
		let lastWheel = 0
		let gestureStartedAtEdge = false
		let distanceSum = 0
		let coolUntil = 0
		const onWheel = (event: WheelEvent) => {
			// the reader moves the view: it no longer sticks to the side it was entered from
			enteredRef.current = null
			if (event.ctrlKey || event.metaKey) {
				event.preventDefault()
				zoomPoint = { x: event.clientX, y: event.clientY }
				zoomRatio = clampRatio(
					zoomRef.current,
					zoomRatio * Math.exp(-event.deltaY / 400),
					MIN_ZOOM,
					MAX_ZOOM,
				)
				const inner = contentRef.current
				if (inner) {
					if (!zoomOrigin || !zoomBounds) {
						const innerBox = inner.getBoundingClientRect()
						zoomOrigin = { x: zoomPoint.x - innerBox.left, y: zoomPoint.y - innerBox.top }
						zoomBounds = { content: innerBox, view: element.getBoundingClientRect() }
					}
					const look = pinchTransform(zoomOrigin, zoomPoint, zoomPoint, zoomRatio, zoomBounds)
					inner.style.transformOrigin = look.transformOrigin
					inner.style.transform = look.transform
				}
				window.clearTimeout(zoomTimeout)
				zoomTimeout = window.setTimeout(() => {
					const innerNow = contentRef.current
					const transform = innerNow?.style.transform ?? ''
					if (innerNow) innerNow.style.transform = ''
					const next = clampZoom(zoomRef.current * zoomRatio)
					zoomAtRef.current(next, zoomPoint)
					if (innerNow && transform && next !== zoomRef.current)
						innerNow.style.transform = transform
					zoomRatio = 1
					zoomOrigin = null
					zoomBounds = null
				}, 140)
				return
			}
			const now = performance.now()
			const newGesture = now - lastWheel > WHEEL_GESTURE_GAP_MS
			lastWheel = now
			if (now < coolUntil || Math.abs(event.deltaY) < Math.abs(event.deltaX)) return
			const down = event.deltaY > 0
			const maxTop = element.scrollHeight - element.clientHeight
			const atEdge = down ? element.scrollTop >= maxTop - 1 : element.scrollTop <= 1
			if (maxTop > 1) {
				// a page bigger than the screen scrolls; at its end, the next wheel gesture turns
				if (newGesture) {
					gestureStartedAtEdge = atEdge
					distanceSum = 0
				}
				if (!atEdge || !gestureStartedAtEdge) return
			} else if (newGesture) {
				distanceSum = 0
			}
			distanceSum += Math.abs(event.deltaY)
			if (distanceSum < WHEEL_TURN_DISTANCE) return
			distanceSum = 0
			coolUntil = now + 400
			turnRef.current(down ? forwardRef.current : (-forwardRef.current as 1 | -1))
		}
		element.addEventListener('wheel', onWheel, { passive: false })
		return () => {
			window.clearTimeout(zoomTimeout)
			element.removeEventListener('wheel', onWheel)
		}
	}, [])

	// Looking at pages for crop ---------------------------------------------------------------------
	const imagesRef = useRef(new Map<number, HTMLImageElement>())
	const wantsAnalysis = !!cropBorders
	const onImageLoad = useCallback(
		(page: number, size: { width: number; height: number }) => {
			setPageSize(page, { ...size, ratio: size.width / size.height })
			const image = imagesRef.current.get(page)
			if (wantsAnalysis && image) analysePage(book.id, page + 1, image, true)
		},
		[setPageSize, wantsAnalysis, book.id],
	)

	// The views next to this one, drawn invisibly so a turn shows a decoded image right away
	const neighbours = useMemo(() => {
		const pages = new Set<number>()
		for (const index of [viewIndex - 1, viewIndex + 1]) {
			views[index]?.forEach((item) => pages.add(item.page))
		}
		items.forEach((item) => pages.delete(item.page))
		return [...pages]
	}, [views, viewIndex, items])

	const touchAction =
		overflowX && overflowY ? 'pan-x pan-y' : overflowX ? 'pan-x' : overflowY ? 'pan-y' : 'none'

	return (
		<>
			{/* around the pages: the reader theme's paper */}
			<div aria-hidden className="inset-0 fixed bg-background" />
			<div
				ref={scrollRef}
				data-testid="comic-pages"
				data-view={viewIndex}
				data-zoom={zoom}
				className="fixed overflow-auto overscroll-none select-none [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
				style={{
					top: 'env(safe-area-inset-top)',
					right: 'env(safe-area-inset-right)',
					bottom: 'env(safe-area-inset-bottom)',
					left: 'env(safe-area-inset-left)',
					touchAction,
				}}
				onPointerDown={onPointerDown}
				onPointerUp={onPointerUp}
				onPointerCancel={onPointerCancel}
				onScroll={() => {
					if (pressRef.current) pressRef.current.cancelled = true
				}}
			>
				<div
					className="flex items-center justify-center"
					style={{
						width: Math.max(content.w, viewport.w),
						height: Math.max(content.h, viewport.h),
					}}
				>
					<div
						ref={contentRef}
						className="flex shrink-0 flex-row"
						style={{
							width: geometry ? content.w : '100%',
							height: geometry ? content.h : '100%',
							filter: brightness < 1 ? `brightness(${brightness * 100}%)` : undefined,
						}}
					>
						{items.map((item, index) => (
							<PageImage
								key={`${item.page}-${item.part}`}
								item={item}
								src={getPageUrl(item.page + 1)}
								box={geometry?.boxes[index]}
								natural={imageSizes[item.page]}
								height={geometry ? content.h : undefined}
								onLoad={onImageLoad}
								imageRef={(image) => {
									if (image) imagesRef.current.set(item.page, image)
									else imagesRef.current.delete(item.page)
								}}
							/>
						))}
					</div>
				</div>
			</div>
			{neighbours.map((page) => (
				<EntityImage
					key={`next-${page}`}
					src={getPageUrl(page + 1)}
					aria-hidden
					alt=""
					className="top-0 left-0 pointer-events-none fixed -z-20 h-full w-auto opacity-0"
					onLoad={(size) => setPageSize(page, { ...size, ratio: size.width / size.height })}
				/>
			))}
		</>
	)
}

type PageImageProps = {
	item: ViewItem
	src: string
	/** The part of the image to show (cropped, a half); unknown until its size is */
	box?: Box
	natural?: { width: number; height: number }
	/** The view's drawn height; unset until the page's size is known */
	height?: number
	onLoad: (page: number, size: { width: number; height: number }) => void
	imageRef: (image: HTMLImageElement | null) => void
}

/**
 * One page, or the part of it the view shows: a box of the page's own size scaled to the
 * view's height, the image placed inside it so only the box shows (a crop or a half, without
 * drawing to a canvas)
 */
const PageImage = memo(function PageImage({
	item,
	src,
	box,
	natural,
	height,
	onLoad,
	imageRef,
}: PageImageProps) {
	const handleLoad = useCallback(
		(size: { width: number; height: number }) => onLoad(item.page, size),
		[onLoad, item.page],
	)
	if (!box || !natural || !height) {
		// the size isn't known yet: the whole page, fitted, until it loads
		return (
			<EntityImage
				ref={imageRef}
				src={src}
				alt=""
				draggable={false}
				decoding="async"
				className="h-full w-full object-contain"
				onLoad={handleLoad}
				onError={(event) => {
					;(event.target as HTMLImageElement).src = '/favicon.png'
				}}
			/>
		)
	}
	const scale = height / box.h
	return (
		<div
			className="relative shrink-0 overflow-hidden"
			style={{ width: box.w * scale, height }}
			data-page={item.page + 1}
			data-part={item.part}
		>
			<EntityImage
				ref={imageRef}
				src={src}
				alt=""
				draggable={false}
				decoding="async"
				className="absolute max-h-none max-w-none"
				style={{
					width: natural.width * scale,
					height: natural.height * scale,
					left: -box.x * scale,
					top: -box.y * scale,
				}}
				onLoad={handleLoad}
				onError={(event) => {
					;(event.target as HTMLImageElement).src = '/favicon.png'
				}}
			/>
		</div>
	)
})

export default memo(PagedReader)
