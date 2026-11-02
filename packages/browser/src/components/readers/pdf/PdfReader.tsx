import { useSDK } from '@stump/client'
import { FullScreenLoader, Text } from '@stump/components'
import type { BookReaderSceneQuery } from '@stump/graphql'
import type { PdfLayout, PdfSearchResult } from '@stump/sdk'
import { useQueryClient } from '@tanstack/react-query'
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import { Link } from 'react-router'
import { toast } from 'sonner'

import { useTheme } from '@/hooks/useTheme'
import { useTranslate } from '@/hooks/useTranslate'
import { usePaths } from '@/paths'
import { useBookPreferences } from '@/scenes/book/reader/useBookPreferences'
import { useReaderStore } from '@/stores'
import { useBookTimer } from '@/stores/reader'

import {
	AnnotationNoteDialog,
	type AnnotationTarget,
	AnnotationToolbar,
	isEditableTarget,
	isOptimisticAnnotationId,
	type ReaderAnnotation,
	type ReaderLocator,
	type ShortcutRow,
	type ToolbarAnchor,
} from '../annotations'
import { useReaderChromeTheme } from '../epub/readium/useReaderChromeTheme'
import { navigationTapZone, swipeOf, swipeTurn, zoneTurn } from '../shared/gestures'
import {
	canShowTwoPages,
	effectiveLayout,
	pageIndicatorText,
	type ReaderLayout,
	rowOfPage,
	spreadRows,
} from '../shared/layout'
import { PAN_STEP, panOrTurn, SCREEN_STEP, ZOOMED_EPSILON } from '../shared/panKeys'
import { copyText, nextDayNightTheme } from '../shared/readerKeys'
import ReaderPageIndicator from '../shared/ReaderPageIndicator'
import ReaderSettingsDialog from '../shared/ReaderSettingsDialog'
import ReaderTopBar from '../shared/ReaderTopBar'
import { SwitchRow } from '../shared/settingsControls'
import { useReaderChrome } from '../shared/useReaderChrome'
import {
	anchorOnPage,
	clampRatio,
	distance,
	DOUBLE_TAP_MS,
	isDoubleTap,
	midpoint,
	pinchTransform,
	type Point,
	scrollForAnchor,
	type ZoomAnchor,
} from '../shared/zoomAnchor'
import {
	boundingRect,
	clampZoom,
	firstPageHint,
	type FitMode,
	type FractionRect,
	MAX_ZOOM,
	MIN_ZOOM,
	pageCssSize,
	pickTier,
	rectContains,
	zoomIn,
	zoomOut,
} from './geometry'
import { isZoomedIn, scrollerTouchStyle } from './gestures'
import { chapterTitleForPage, flattenOutline, pageLabel, resolvePageInput } from './outline'
import PdfGoToPageDialog from './PdfGoToPageDialog'
import PdfLocationsDialog, { type PdfLocationTab } from './PdfLocationsDialog'
import PdfPageView from './PdfPageView'
import { PDF_READER_CSS } from './pdfReaderCss'
import PdfSearchDialog from './PdfSearchDialog'
import { PdfOpenAs, PdfPageSettings } from './PdfSettings'
import { resolvePdfShortcut } from './pdfShortcuts'
import { pdfThemeLook, resolvePdfTheme } from './pdfThemes'
import {
	buildScrollLayout,
	dominantPage,
	mountRange,
	prefetchIndexes,
	progressionWithin,
	scrollTopFor,
	visibleRange,
} from './scrollLayout'
import { PAGE_ATTRIBUTE, readPdfSelection, selectionToLocator, totalProgression } from './selection'
import { type PdfBookPreferences, usePdfBookPreferences, usePdfReaderStore } from './store'
import { buildPageString, pagePreview } from './textLayout'
import { useElementSize } from './useElementSize'
import {
	pdfTextQueryKey,
	usePdfAnnotations,
	usePdfBookmarks,
	usePdfLayout,
	usePrefetchPdfText,
} from './usePdfData'

export type PdfReaderBook = NonNullable<BookReaderSceneQuery['mediaById']>

type Props = {
	media: PdfReaderBook
	isIncognito?: boolean
	/** 1-based page to open at; defaults to the saved progress */
	initialPage?: number
	onProgress?: (page: number, elapsedSeconds: number) => void
}

/**
 * The PDF reader: server-rendered page pictures sized for the screen, with the page's text
 * laid over them so it can be selected, searched and annotated like an EPUB. Pages scroll
 * continuously (only those near the viewport are mounted) or one at a time.
 */
export default function PdfReader(props: Props) {
	const { translate } = useTranslate()
	const paths = usePaths()
	const preferences = usePdfBookPreferences(props.media.id)
	// Ask for the first page's picture with the layout (phones fit the width by default), so
	// the server renders it while the layout travels back. Computed once: it only matters
	// for the first request.
	const [firstPage] = useState(() => {
		if (typeof window === 'undefined') return undefined
		const viewportWidth = window.innerWidth
		const isPhone = viewportWidth < PHONE_MAX_WIDTH
		return firstPageHint({
			page: props.initialPage ?? props.media.readProgress?.page ?? 1,
			viewportWidth,
			devicePixelRatio: Math.min(2, window.devicePixelRatio || 1),
			fit: preferences.fit ?? (isPhone ? 'width' : 'page'),
			zoom: preferences.zoom ?? 1,
			sidePadding: isPhone ? 0 : PAGE_GAP_DESKTOP,
		})
	})
	const { data: layout, isLoading, error, refetch } = usePdfLayout(props.media.id, firstPage)
	const setBookPreferences = usePdfReaderStore((state) => state.setBookPreferences)

	if (isLoading) {
		return <FullScreenLoader />
	}

	if (error || !layout) {
		return (
			<div className="gap-3 p-6 flex h-full w-full flex-col items-center justify-center text-center">
				<Text>{translate('shared.pdfReader.openFailed')}</Text>
				<div className="gap-2 flex flex-wrap justify-center">
					<button type="button" className="text-sm underline" onClick={() => void refetch()}>
						{translate('shared.pdfReader.retry')}
					</button>
					<button
						type="button"
						className="text-sm underline"
						onClick={() => setBookPreferences(props.media.id, { reader: 'pages' })}
					>
						{translate('shared.pdfReader.openAsPages')}
					</button>
					<Link className="text-sm underline" to={paths.bookOverview(props.media.id)}>
						{translate('shared.imageReader.header.goToMediaOverview')}
					</Link>
				</div>
			</div>
		)
	}

	return <PdfReaderView {...props} layout={layout} />
}

type ViewProps = Props & { layout: PdfLayout }

type NoteDialogState =
	| { mode: 'create'; locator: ReaderLocator; quotedText: string; style: ReaderAnnotation['style'] }
	| { mode: 'edit'; annotation: ReaderAnnotation }

type Flash = { page: number; rects: FractionRect[]; key: number }

const PAGE_GAP_DESKTOP = 16
const PAGE_GAP_PHONE = 8
const PHONE_MAX_WIDTH = 768
/** Taps right after a pinch or a pan are the end of that gesture, not a tap */
const GESTURE_QUIET_MS = 350

function PdfReaderView({ media, layout, isIncognito = false, initialPage, onProgress }: ViewProps) {
	const { sdk } = useSDK()
	const { translate } = useTranslate()
	const paths = usePaths()
	const queryClient = useQueryClient()
	const mediaId = media.id
	const pageCount = layout.pageCount

	// Preferences ------------------------------------------------------------------------
	const preferences = usePdfBookPreferences(mediaId)
	const storeSetPreferences = usePdfReaderStore((state) => state.setBookPreferences)
	const setPreferences = useCallback(
		(changes: Partial<PdfBookPreferences>) => storeSetPreferences(mediaId, changes),
		[mediaId, storeSetPreferences],
	)
	const themeSetting = useReaderStore((state) => state.settings.theme ?? 'auto')
	const setReaderSettings = useReaderStore((state) => state.setSettings)
	const { isDarkVariant } = useTheme()
	const theme = resolvePdfTheme(themeSetting, isDarkVariant)
	const invertPagesInDark = preferences.invertPagesInDark ?? true
	const look = useMemo(() => pdfThemeLook(theme, invertPagesInDark), [theme, invertPagesInDark])
	useReaderChromeTheme(themeSetting)

	const {
		bookPreferences: { trackElapsedTime },
	} = useBookPreferences({ book: media })
	const timer = useBookTimer(mediaId, {
		initial: media.readProgress?.elapsedSeconds ?? undefined,
		enabled: trackElapsedTime,
	})

	// Viewport and page sizes ---------------------------------------------------------------
	const scrollRef = useRef<HTMLDivElement>(null)
	const contentRef = useRef<HTMLDivElement>(null)
	const viewport = useElementSize(scrollRef)
	const isPhone = viewport.width > 0 && viewport.width < PHONE_MAX_WIDTH
	const isCoarsePointer = useMemo(
		() => typeof window !== 'undefined' && !!window.matchMedia?.('(pointer: coarse)').matches,
		[],
	)
	const fit: FitMode = preferences.fit ?? (isPhone ? 'width' : 'page')
	const zoom = clampZoom(preferences.zoom ?? 1)
	/** Zoomed in past the fit: a finger pans the page; at the fit it only scrolls vertically */
	const zoomedIn = isZoomedIn(zoom)
	const touchStyle = scrollerTouchStyle(zoomedIn)
	const storedLayout = useMemo<ReaderLayout>(
		() => ({
			spread: preferences.spread ?? 'single',
			flow: preferences.scrollMode === 'paged' ? 'paged' : 'continuous',
		}),
		[preferences.spread, preferences.scrollMode],
	)
	const shownLayout = effectiveLayout(storedLayout, viewport.width)
	const continuous = shownLayout.flow === 'continuous'
	const double = shownLayout.spread === 'double'
	const coverAlone = preferences.coverAlone ?? true
	const gap = isPhone ? PAGE_GAP_PHONE : PAGE_GAP_DESKTOP
	const sidePadding = isPhone ? 0 : PAGE_GAP_DESKTOP

	const available = useMemo(
		() => ({
			// two pages share the width, with a gap between them
			width: Math.max(
				0,
				(viewport.width - sidePadding * 2 - (double ? gap : 0)) / (double ? 2 : 1),
			),
			height: Math.max(0, viewport.height - gap * 2),
		}),
		[viewport.width, viewport.height, sidePadding, gap, double],
	)
	const sizes = useMemo(
		() => layout.pages.map((page) => pageCssSize(page, available, fit, zoom)),
		[layout.pages, available, fit, zoom],
	)
	const rows = useMemo(
		() => spreadRows(pageCount, shownLayout.spread, { coverAlone }),
		[pageCount, shownLayout.spread, coverAlone],
	)
	const scrollLayout = useMemo(() => buildScrollLayout(sizes, gap, rows), [sizes, gap, rows])
	const contentWidth = Math.max(viewport.width, scrollLayout.maxRowWidth + sidePadding * 2)
	// Pictures sharper than 2x don't read better on a phone, they only load slower
	const dpr = Math.min(2, typeof window !== 'undefined' ? window.devicePixelRatio || 1 : 1)

	/** Where a row starts horizontally in the content (rows are centred) */
	const rowLeft = useCallback(
		(row: number) =>
			Math.max(sidePadding, (contentWidth - (scrollLayout.rows[row]?.width ?? 0)) / 2),
		[sidePadding, contentWidth, scrollLayout.rows],
	)

	// Position ------------------------------------------------------------------------------
	const startIndex = useMemo(() => {
		const page = initialPage ?? media.readProgress?.page ?? 1
		return Math.min(pageCount, Math.max(1, page || 1)) - 1
	}, [initialPage, media.readProgress?.page, pageCount])

	const [scrollTop, setScrollTop] = useState(0)
	const [pagedIndex, setPagedIndex] = useState(startIndex)
	/** The reading position kept across zoom, fit, layout and resize changes */
	const anchorRef = useRef({ index: startIndex, progression: 0 })
	/** A spot to keep under the fingers or the mouse when a zoom is committed */
	const zoomAnchorRef = useRef<ZoomAnchor | null>(null)
	const programmaticScrollRef = useRef(false)
	/** When the last pinch or pan ended (taps right after it are ignored) */
	const lastGestureRef = useRef(0)

	/** The row shown in paged layouts */
	const pagedRow = Math.max(0, rowOfPage(rows, pagedIndex))
	const currentIndex = continuous
		? viewport.height > 0
			? dominantPage(scrollLayout, scrollTop, viewport.height)
			: startIndex
		: (rows[pagedRow]?.[0] ?? pagedIndex)
	const currentPage = currentIndex + 1
	const currentRow = Math.max(0, rowOfPage(rows, currentIndex))
	const pagesInView = useMemo(
		() => (rows[currentRow] ?? [currentIndex]).map((index) => index + 1),
		[rows, currentRow, currentIndex],
	)

	/** The top of a page in the content, vertically centred in its row */
	const pageTop = useCallback(
		(index: number) => {
			const slot = scrollLayout.slots[index]
			if (!slot) return 0
			const row = scrollLayout.rows[slot.row]!
			const rowTop = continuous ? row.top : Math.max(gap, (viewport.height - row.height) / 2)
			return rowTop + (row.height - slot.height) / 2
		},
		[scrollLayout, continuous, gap, viewport.height],
	)
	const pageLeft = useCallback(
		(index: number) => {
			const slot = scrollLayout.slots[index]
			return slot ? rowLeft(slot.row) + slot.left : 0
		},
		[scrollLayout.slots, rowLeft],
	)

	// Restore the reading position whenever the layout changes (first paint, zoom, fit,
	// resize, layout); a committed pinch or wheel zoom keeps its spot under the fingers
	useLayoutEffect(() => {
		const element = scrollRef.current
		if (!element || viewport.height <= 0) return
		const zoomAnchor = zoomAnchorRef.current
		zoomAnchorRef.current = null
		if (contentRef.current) {
			contentRef.current.style.transform = ''
			contentRef.current.style.transformOrigin = ''
		}
		if (zoomAnchor) {
			const index = zoomAnchor.page - 1
			const slot = scrollLayout.slots[index]
			if (slot) {
				if (!continuous) setPagedIndex(index)
				const { left, top } = scrollForAnchor(zoomAnchor, {
					left: pageLeft(index),
					top: pageTop(index),
					width: slot.width,
					height: slot.height,
				})
				programmaticScrollRef.current = true
				element.scrollLeft = left
				element.scrollTop = top
				setScrollTop(element.scrollTop)
				const dominant = continuous
					? dominantPage(scrollLayout, element.scrollTop, element.clientHeight)
					: index
				anchorRef.current = {
					index: dominant,
					progression: continuous
						? progressionWithin(scrollLayout, dominant, element.scrollTop)
						: 0,
				}
				return
			}
		}
		if (continuous) {
			const { index, progression } = anchorRef.current
			programmaticScrollRef.current = true
			element.scrollTop = scrollTopFor(scrollLayout, index, progression)
			setScrollTop(element.scrollTop)
		} else {
			setPagedIndex(anchorRef.current.index)
		}
	}, [scrollLayout, continuous, viewport.height, pageLeft, pageTop])

	const frameRef = useRef<number | null>(null)
	const handleScroll = useCallback(() => {
		if (frameRef.current != null) return
		frameRef.current = window.requestAnimationFrame(() => {
			frameRef.current = null
			const element = scrollRef.current
			if (!element || !continuous) return
			const top = element.scrollTop
			setScrollTop(top)
			if (programmaticScrollRef.current) {
				programmaticScrollRef.current = false
				return
			}
			const index = dominantPage(scrollLayout, top, element.clientHeight)
			const row = scrollLayout.rows[scrollLayout.slots[index]?.row ?? -1]
			anchorRef.current = {
				index,
				progression: row && top > row.top ? progressionWithin(scrollLayout, index, top) : 0,
			}
		})
	}, [continuous, scrollLayout])

	useEffect(
		() => () => {
			if (frameRef.current != null) window.cancelAnimationFrame(frameRef.current)
		},
		[],
	)

	/**
	 * Goes to a 1-based page. With a `target` (0-1 down the page, e.g. a search hit or an
	 * annotation) the target is shown a third of the way down the screen; otherwise the
	 * page's top is.
	 */
	const goToPage = useCallback(
		(page: number, target?: number) => {
			const index = Math.min(pageCount - 1, Math.max(0, Math.round(page) - 1))
			const slot = scrollLayout.slots[index]
			const row = slot ? scrollLayout.rows[slot.row] : undefined
			const element = scrollRef.current
			let within = 0
			if (target != null && target > 0 && slot && row && element) {
				const targetY =
					(pageTop(index) - (continuous ? row.top : 0) + target * slot.height) / row.height
				within = Math.max(0, targetY - (element.clientHeight * 0.3) / Math.max(1, row.height))
			}
			anchorRef.current = { index, progression: within }
			if (continuous) {
				if (!element) return
				programmaticScrollRef.current = true
				element.scrollTop = scrollTopFor(scrollLayout, index, within)
				setScrollTop(element.scrollTop)
			} else {
				setPagedIndex(index)
				// a zoomed page taller than the screen: scroll to the target inside it
				element?.scrollTo({
					top:
						target != null && slot
							? Math.max(0, pageTop(index) + target * slot.height - element.clientHeight * 0.3)
							: 0,
				})
			}
		},
		[continuous, pageCount, scrollLayout, pageTop],
	)

	/** Moves by rows: a page, or a spread of two */
	const goRelative = useCallback(
		(delta: number) => {
			const target = rows[Math.min(rows.length - 1, Math.max(0, currentRow + delta))]
			if (target) goToPage(target[0]! + 1)
		},
		[rows, currentRow, goToPage],
	)

	const scrollByScreen = useCallback((direction: 1 | -1) => {
		const element = scrollRef.current
		if (!element) return
		element.scrollBy({ top: direction * element.clientHeight * 0.9 })
	}, [])

	// Progress ------------------------------------------------------------------------------
	const getElapsedSeconds = timer.getCurrentTime
	useEffect(() => {
		if (isIncognito || !onProgress) return undefined
		const timeout = window.setTimeout(() => onProgress(currentPage, getElapsedSeconds()), 700)
		return () => window.clearTimeout(timeout)
	}, [currentPage, isIncognito, onProgress, getElapsedSeconds])

	// Pictures ------------------------------------------------------------------------------
	/** Prefetched pictures, by page and tier, so a mounted page reuses the cached response */
	const [prefetched, setPrefetched] = useState<ReadonlyMap<string, string>>(() => new Map())
	const requestedRef = useRef(new Set<string>())
	const tierFor = useCallback(
		(index: number) => pickTier(sizes[index]?.width ?? 0, dpr, layout.tiers),
		[sizes, dpr, layout.tiers],
	)
	const srcFor = useCallback(
		(index: number) => {
			const tier = tierFor(index)
			return (
				prefetched.get(`${index}:${tier}`) ??
				sdk.pdf.pageURL(mediaId, index + 1, { w: tier, v: layout.v })
			)
		},
		[tierFor, prefetched, sdk, mediaId, layout.v],
	)

	/**
	 * The pages in view load first: the other mounted pages and the prefetches wait until
	 * these pictures are up, so on a slow link they don't share its bandwidth
	 */
	const visible: [number, number] | null = continuous
		? viewport.height > 0
			? visibleRange(scrollLayout, scrollTop, viewport.height)
			: null
		: rows[pagedRow]
			? [rows[pagedRow]![0]!, rows[pagedRow]![rows[pagedRow]!.length - 1]!]
			: null
	const [loadedPictures, setLoadedPictures] = useState<ReadonlySet<string>>(() => new Set())
	const handlePictureLoaded = useCallback((src: string) => {
		setLoadedPictures((previous) => (previous.has(src) ? previous : new Set(previous).add(src)))
	}, [])
	let visibleLoaded = !!visible
	if (visible) {
		for (let index = visible[0]; index <= visible[1]; index++) {
			if (!loadedPictures.has(srcFor(index))) {
				visibleLoaded = false
				break
			}
		}
	}

	const prefetchText = usePrefetchPdfText(mediaId, layout.v)
	const lastInView = visible ? visible[1] : currentIndex
	useEffect(() => {
		if (viewport.width <= 0 || !visibleLoaded) return undefined
		// a spread prefetches the next spread and the previous one
		const indexes = double
			? [
					...prefetchIndexes(lastInView, pageCount, 2, 0),
					...prefetchIndexes(currentIndex, pageCount, 0, 2),
				]
			: prefetchIndexes(currentIndex, pageCount)
		const run = () => {
			for (const index of indexes) {
				const tier = tierFor(index)
				const key = `${index}:${tier}`
				if (!requestedRef.current.has(key)) {
					requestedRef.current.add(key)
					const url = sdk.pdf.pageURL(mediaId, index + 1, { w: tier, v: layout.v, prefetch: true })
					const image = new Image()
					image.decoding = 'async'
					image.onload = () => setPrefetched((previous) => new Map(previous).set(key, url))
					// a dropped prefetch (503) is asked for again later
					image.onerror = () => requestedRef.current.delete(key)
					image.src = url
				}
				if (layout.hasText) void prefetchText(index + 1)
			}
		}
		const idle = window.requestIdleCallback
		if (idle) {
			const handle = idle(run, { timeout: 800 })
			return () => window.cancelIdleCallback?.(handle)
		}
		const timeout = window.setTimeout(run, 150)
		return () => window.clearTimeout(timeout)
	}, [
		currentIndex,
		lastInView,
		pageCount,
		tierFor,
		sdk,
		mediaId,
		layout.v,
		layout.hasText,
		prefetchText,
		viewport.width,
		visibleLoaded,
		double,
	])

	// Annotations and bookmarks ----------------------------------------------------------------
	const { annotations, createAnnotation, updateAnnotation, deleteAnnotation } = usePdfAnnotations(
		mediaId,
		isIncognito,
	)
	const annotationsByPage = useMemo(() => {
		const byPage = new Map<number, ReaderAnnotation[]>()
		for (const annotation of annotations) {
			const page = annotation.locator.locations?.position
			if (!page) continue
			const list = byPage.get(page)
			if (list) list.push(annotation)
			else byPage.set(page, [annotation])
		}
		return byPage
	}, [annotations])

	const { bookmarks, createBookmark, deleteBookmark } = usePdfBookmarks(mediaId)
	const currentBookmark = bookmarks.find((bookmark) => bookmark.page === currentPage)

	const flatOutline = useMemo(() => flattenOutline(layout.outline), [layout.outline])
	const labelOf = useCallback((page: number) => pageLabel(layout, page), [layout])

	const pageStringOf = useCallback(
		(page: number) => {
			const text = queryClient.getQueryData<{ runs: Parameters<typeof buildPageString>[0] }>(
				pdfTextQueryKey(mediaId, layout.v, page),
			)
			return text ? buildPageString(text.runs) : null
		},
		[queryClient, mediaId, layout.v],
	)

	const toggleBookmark = useCallback(async () => {
		if (isIncognito) {
			toast.info(translate('shared.pdfReader.bookmarksDisabledInIncognito'))
			return
		}
		if (currentBookmark) {
			deleteBookmark(currentBookmark.id)
			toast.success(translate('shared.readerAnnotations.bookmarkRemoved'))
			return
		}
		const page = currentPage
		let preview = ''
		if (layout.hasText) {
			try {
				const text = await queryClient.fetchQuery({
					queryKey: pdfTextQueryKey(mediaId, layout.v, page),
					queryFn: ({ signal }) => sdk.pdf.text(mediaId, page, layout.v, signal),
					staleTime: Infinity,
				})
				preview = pagePreview(text.runs)
			} catch {
				preview = ''
			}
		}
		const progression = continuous ? progressionWithin(scrollLayout, page - 1, scrollTop) : 0
		createBookmark(
			{
				href: `page/${page}`,
				type: 'application/pdf',
				title: labelOf(page),
				chapterTitle: chapterTitleForPage(flatOutline, page),
				locations: {
					position: page,
					progression,
					totalProgression: totalProgression(page, progression, pageCount),
					fragments: [`page=${page}`],
				},
				text: preview ? { highlight: preview } : undefined,
			},
			preview || translate('shared.readerAnnotations.page', { page: labelOf(page) }),
		)
		toast.success(translate('shared.readerAnnotations.bookmarkAdded'))
	}, [
		isIncognito,
		currentBookmark,
		currentPage,
		deleteBookmark,
		translate,
		layout.hasText,
		layout.v,
		queryClient,
		mediaId,
		sdk,
		continuous,
		scrollLayout,
		scrollTop,
		createBookmark,
		labelOf,
		flatOutline,
		pageCount,
	])

	// Selection toolbar ---------------------------------------------------------------------------
	const [target, setTarget] = useState<AnnotationTarget | null>(null)
	const [noteDialog, setNoteDialog] = useState<NoteDialogState | null>(null)
	const pointerDownRef = useRef(false)
	const targetAnnotation =
		target?.kind === 'annotation'
			? annotations.find((annotation) => annotation.id === target.annotationId)
			: undefined

	const clearSelection = useCallback(() => {
		window.getSelection()?.removeAllRanges()
		setTarget(null)
	}, [setTarget])

	const readSelectionTarget = useCallback((): AnnotationTarget | null => {
		const selection = readPdfSelection(window.getSelection())
		if (!selection) return null
		const locator = selectionToLocator(selection, {
			mediaId,
			pageCount,
			label: layout.labels?.[selection.page - 1] ?? null,
			chapterTitle: chapterTitleForPage(flatOutline, selection.page),
			pageString: pageStringOf(selection.page),
		})
		return {
			kind: 'selection',
			selection: { rect: selection.anchor, locator, text: selection.text },
		}
	}, [mediaId, pageCount, layout.labels, flatOutline, pageStringOf])

	// The toolbar shows once the selection has settled: on touch screens the selection's
	// handles are still being dragged until it's quiet for a moment
	useEffect(() => {
		let timeout: number | undefined
		const update = () => {
			if (pointerDownRef.current) return
			const next = readSelectionTarget()
			setTarget((current) => {
				if (next) return next
				return current?.kind === 'selection' ? null : current
			})
		}
		const onSelectionChange = () => {
			window.clearTimeout(timeout)
			timeout = window.setTimeout(update, isCoarsePointer ? 300 : 150)
		}
		document.addEventListener('selectionchange', onSelectionChange)
		return () => {
			window.clearTimeout(timeout)
			document.removeEventListener('selectionchange', onSelectionChange)
		}
	}, [readSelectionTarget, isCoarsePointer])

	const annotationAnchor = useCallback((annotation: ReaderAnnotation): ToolbarAnchor | null => {
		const page = annotation.locator.locations?.position
		const box = boundingRect(annotation.locator.locations?.rects ?? [])
		const element = page ? document.querySelector(`[data-pdf-page="${page}"]`) : null
		if (!element || !box) return null
		const pageBox = element.getBoundingClientRect()
		return {
			x: pageBox.left + box.x * pageBox.width,
			y: pageBox.top + box.y * pageBox.height,
			width: box.width * pageBox.width,
			height: box.height * pageBox.height,
		}
	}, [])

	const openAnnotation = useCallback(
		(annotationId: string, anchor?: ToolbarAnchor | DOMRect) => {
			const annotation = annotations.find((candidate) => candidate.id === annotationId)
			if (!annotation || isOptimisticAnnotationId(annotation.id)) return
			const rect = annotationAnchor(annotation) ?? anchor
			if (!rect) return
			window.getSelection()?.removeAllRanges()
			setTarget({
				kind: 'annotation',
				annotationId,
				rect: { x: rect.x, y: rect.y, width: rect.width, height: rect.height },
			})
		},
		[annotations, annotationAnchor, setTarget],
	)

	// Keep the toolbar next to its target while scrolling or zooming
	useEffect(() => {
		if (!target) return
		if (target.kind === 'selection') {
			const next = readSelectionTarget()
			if (!next) {
				setTarget(null)
			} else if (
				next.kind === 'selection' &&
				(next.selection.rect.x !== target.selection.rect.x ||
					next.selection.rect.y !== target.selection.rect.y)
			) {
				setTarget(next)
			}
		} else if (targetAnnotation) {
			const rect = annotationAnchor(targetAnnotation)
			if (rect && (rect.x !== target.rect.x || rect.y !== target.rect.y)) {
				setTarget({ ...target, rect })
			}
		}
	}, [
		scrollTop,
		sizes,
		pagedIndex,
		target,
		targetAnnotation,
		readSelectionTarget,
		annotationAnchor,
	])

	/**
	 * What a toolbar button or key acts on, read when it's pressed: the selection's handles
	 * (phones) may have moved since the toolbar showed, without a settled `selectionchange` yet
	 */
	const liveTarget = useCallback(
		(): AnnotationTarget | null =>
			target?.kind === 'selection' ? (readSelectionTarget() ?? target) : target,
		[target, readSelectionTarget],
	)

	const applyStyle = useCallback(
		(style: ReaderAnnotation['style']) => {
			const current = liveTarget()
			if (!current) return
			if (current.kind === 'selection') {
				createAnnotation(current.selection.locator, { style })
				clearSelection()
			} else {
				updateAnnotation(current.annotationId, { style })
			}
		},
		[liveTarget, createAnnotation, updateAnnotation, clearSelection],
	)

	const openNote = useCallback(() => {
		const current = liveTarget()
		if (!current) return
		if (current.kind === 'selection') {
			setNoteDialog({
				mode: 'create',
				locator: current.selection.locator,
				quotedText: current.selection.text,
				style: 'HIGHLIGHT' as ReaderAnnotation['style'],
			})
		} else if (targetAnnotation) {
			setNoteDialog({ mode: 'edit', annotation: targetAnnotation })
		}
	}, [liveTarget, targetAnnotation, setNoteDialog])

	const copyTarget = useCallback(async () => {
		const current = liveTarget()
		if (!current) return
		const text =
			current.kind === 'selection'
				? current.selection.text
				: (targetAnnotation?.locator.text?.highlight ?? '')
		const copied = await copyText(text)
		if (copied) {
			toast.success(translate('shared.readerAnnotations.copied'))
			clearSelection()
		} else {
			toast.error(translate('shared.readerAnnotations.copyFailed'))
		}
	}, [liveTarget, targetAnnotation, translate, clearSelection])

	const removeTargetAnnotation = useCallback(() => {
		if (target?.kind !== 'annotation') return
		deleteAnnotation(target.annotationId)
		setTarget(null)
	}, [target, deleteAnnotation, setTarget])

	// Dialogs and chrome --------------------------------------------------------------------------
	const [settingsOpen, setSettingsOpen] = useState(false)
	const [settingsSection, setSettingsSection] = useState<'shortcuts' | null>(null)
	const [searchOpen, setSearchOpen] = useState(false)
	const [goToOpen, setGoToOpen] = useState(false)
	const [locationsOpen, setLocationsOpen] = useState(false)
	const [locationsTab, setLocationsTab] = useState<PdfLocationTab>('contents')
	const [flash, setFlash] = useState<Flash | null>(null)
	const chrome = useReaderChrome({
		pinned: settingsOpen || searchOpen || goToOpen || locationsOpen,
	})

	const openSettings = useCallback(
		(section: 'shortcuts' | null = null) => {
			setSettingsSection(section)
			setSettingsOpen(true)
		},
		[setSettingsSection, setSettingsOpen],
	)

	const showSearchResult = useCallback(
		(result: PdfSearchResult) => {
			setSearchOpen(false)
			goToPage(result.page, result.rects[0]?.y ?? 0)
			setFlash({ page: result.page, rects: result.rects, key: Date.now() })
		},
		[goToPage, setSearchOpen, setFlash],
	)

	useEffect(() => {
		if (!flash) return undefined
		const timeout = window.setTimeout(() => setFlash(null), 2600)
		return () => window.clearTimeout(timeout)
	}, [flash])

	// Zoom ----------------------------------------------------------------------------------------
	/** The spot of the page under a viewport point, to keep there when the zoom changes */
	const anchorAt = useCallback((point: Point): ZoomAnchor | null => {
		const container = scrollRef.current
		if (!container) return null
		const containerBox = container.getBoundingClientRect()
		const pagesShown = container.querySelectorAll<HTMLElement>(`[${PAGE_ATTRIBUTE}]`)
		let best: { page: number; box: DOMRect; distance: number } | null = null
		for (const element of pagesShown) {
			const box = element.getBoundingClientRect()
			const dx =
				point.x < box.left ? box.left - point.x : point.x > box.right ? point.x - box.right : 0
			const dy =
				point.y < box.top ? box.top - point.y : point.y > box.bottom ? point.y - box.bottom : 0
			const distance = Math.hypot(dx, dy)
			if (!best || distance < best.distance) {
				best = { page: Number(element.getAttribute(PAGE_ATTRIBUTE)), box, distance }
			}
		}
		if (!best) return null
		return anchorOnPage(best.page, best.box, point, { x: containerBox.left, y: containerBox.top })
	}, [])

	/** Commits a zoom, keeping the spot under `point` (a pinch, the mouse, a double tap) where it is */
	const zoomAround = useCallback(
		(nextZoom: number, point?: Point) => {
			const clamped = clampZoom(nextZoom)
			const content = contentRef.current
			const transform = content?.style.transform ?? ''
			// measure the pages where they really are, without the gesture's transform
			if (content) content.style.transform = ''
			if (clamped === zoom) return
			zoomAnchorRef.current = point ? anchorAt(point) : null
			// kept until the new zoom is laid out (in the same frame), so nothing jumps
			if (content && transform) content.style.transform = transform
			setPreferences({ zoom: clamped })
		},
		[zoom, anchorAt, setPreferences],
	)
	const zoomAroundRef = useRef(zoomAround)
	useEffect(() => {
		zoomAroundRef.current = zoomAround
	}, [zoomAround])
	const anchorAtRef = useRef(anchorAt)
	useEffect(() => {
		anchorAtRef.current = anchorAt
	}, [anchorAt])

	// Keyboard ----------------------------------------------------------------------------------
	useEffect(() => {
		const onKeyDown = (event: KeyboardEvent) => {
			if (event.defaultPrevented || isEditableTarget(event.target)) return
			// any open dialog (read from the DOM: state set by a closing dialog may not have
			// re-rendered yet when the next key arrives)
			if (document.querySelector('[role="dialog"][data-state="open"]')) return
			const action = resolvePdfShortcut(event, {
				hasTarget: !!target,
				targetIsAnnotation: target?.kind === 'annotation',
				continuous,
				zoomedPage: !continuous && zoom > 1 + ZOOMED_EPSILON,
			})
			if (!action) return
			event.preventDefault()
			switch (action.type) {
				case 'style':
					applyStyle(action.style)
					break
				case 'note':
					openNote()
					break
				case 'copy':
					void copyTarget()
					break
				case 'delete':
					removeTargetAnnotation()
					break
				case 'bookmark':
					void toggleBookmark()
					break
				case 'help':
					openSettings('shortcuts')
					break
				case 'dayNight':
					setReaderSettings({ theme: nextDayNightTheme(themeSetting, isDarkVariant) })
					break
				case 'dismiss':
					clearSelection()
					break
				case 'previous':
					goRelative(-1)
					break
				case 'next':
					goRelative(1)
					break
				case 'screenUp':
					scrollByScreen(-1)
					break
				case 'screenDown':
					scrollByScreen(1)
					break
				case 'first':
					goToPage(1)
					break
				case 'last':
					goToPage(pageCount)
					break
				case 'zoomIn':
					setPreferences({ zoom: zoomIn(zoom) })
					break
				case 'zoomOut':
					setPreferences({ zoom: zoomOut(zoom) })
					break
				case 'zoomReset':
					setPreferences({ zoom: 1 })
					break
				case 'toggleFit':
					setPreferences({ fit: fit === 'width' ? 'page' : 'width', zoom: 1 })
					break
				case 'goToPage':
					setGoToOpen(true)
					break
				case 'pan': {
					const element = scrollRef.current
					if (!element) break
					const result = panOrTurn(
						element,
						action.direction,
						action.screen ? SCREEN_STEP : PAN_STEP,
					)
					if (result.type === 'pan') {
						element.scrollTo({
							left: result.left,
							top: result.top,
							behavior: event.repeat ? 'auto' : 'smooth',
						})
					} else {
						goRelative(result.delta)
						// the next page from its start, the previous one from its end
						window.requestAnimationFrame(() =>
							element.scrollTo(
								result.delta > 0
									? { left: 0, top: 0 }
									: { left: element.scrollWidth, top: element.scrollHeight },
							),
						)
					}
					break
				}
				case 'search':
					if (layout.hasText) setSearchOpen(true)
					break
			}
		}
		window.addEventListener('keydown', onKeyDown)
		return () => window.removeEventListener('keydown', onKeyDown)
	}, [
		target,
		continuous,
		applyStyle,
		openNote,
		copyTarget,
		removeTargetAnnotation,
		toggleBookmark,
		openSettings,
		setReaderSettings,
		themeSetting,
		isDarkVariant,
		clearSelection,
		goRelative,
		scrollByScreen,
		goToPage,
		pageCount,
		setPreferences,
		zoom,
		fit,
		layout.hasText,
	])

	// Ctrl/Cmd + wheel (and a trackpad pinch) zooms the pages around the mouse: scaled with a
	// transform while the wheel turns, committed once it stops
	useEffect(() => {
		const element = scrollRef.current
		if (!element) return undefined
		let ratio = 1
		let origin: Point | null = null
		let point: Point = { x: 0, y: 0 }
		let timeout: number | undefined
		const onWheel = (event: WheelEvent) => {
			if (!event.ctrlKey && !event.metaKey) return
			event.preventDefault()
			const content = contentRef.current
			const current = clampZoom(usePdfReaderStore.getState().books[mediaId]?.zoom ?? 1)
			point = { x: event.clientX, y: event.clientY }
			if (!origin) {
				const box = element.getBoundingClientRect()
				origin = {
					x: element.scrollLeft + event.clientX - box.left,
					y: element.scrollTop + event.clientY - box.top,
				}
			}
			ratio = clampRatio(current, ratio * Math.exp(-event.deltaY / 400), MIN_ZOOM, MAX_ZOOM)
			if (content) {
				const look = pinchTransform(origin, point, point, ratio)
				content.style.transformOrigin = look.transformOrigin
				content.style.transform = look.transform
			}
			window.clearTimeout(timeout)
			timeout = window.setTimeout(() => {
				zoomAroundRef.current(current * ratio, point)
				ratio = 1
				origin = null
			}, 140)
		}
		element.addEventListener('wheel', onWheel, { passive: false })
		return () => {
			window.clearTimeout(timeout)
			element.removeEventListener('wheel', onWheel)
		}
	}, [mediaId])

	// Pinch to zoom on touch screens: during the pinch only a GPU transform scales the pages
	// around the fingers' midpoint (no re-render per move); when the fingers lift the zoom is
	// committed once and the spot that was under the fingers stays there. One-finger pans are
	// left to the browser's own momentum scrolling.
	useEffect(() => {
		const element = scrollRef.current
		if (!element) return undefined
		let start: { distance: number; mid: Point; origin: Point; zoom: number } | null = null
		let ratio = 1
		let mid: Point = { x: 0, y: 0 }
		const touchPoint = (touch: Touch): Point => ({ x: touch.clientX, y: touch.clientY })
		const onTouchStart = (event: TouchEvent) => {
			if (event.touches.length !== 2) return
			const a = touchPoint(event.touches[0]!)
			const b = touchPoint(event.touches[1]!)
			mid = midpoint(a, b)
			const box = element.getBoundingClientRect()
			start = {
				distance: Math.max(1, distance(a, b)),
				mid,
				origin: {
					x: element.scrollLeft + mid.x - box.left,
					y: element.scrollTop + mid.y - box.top,
				},
				zoom: clampZoom(usePdfReaderStore.getState().books[mediaId]?.zoom ?? 1),
			}
			ratio = 1
			// the text selection a long press may have started is not what a pinch wants
			window.getSelection()?.removeAllRanges()
			if (contentRef.current) contentRef.current.style.willChange = 'transform'
		}
		const onTouchMove = (event: TouchEvent) => {
			if (!start || event.touches.length !== 2) return
			event.preventDefault()
			const a = touchPoint(event.touches[0]!)
			const b = touchPoint(event.touches[1]!)
			mid = midpoint(a, b)
			ratio = clampRatio(start.zoom, distance(a, b) / start.distance, MIN_ZOOM, MAX_ZOOM)
			const content = contentRef.current
			if (content) {
				const look = pinchTransform(start.origin, start.mid, mid, ratio)
				content.style.transformOrigin = look.transformOrigin
				content.style.transform = look.transform
			}
		}
		const onTouchEnd = (event: TouchEvent) => {
			if (!start || event.touches.length > 0) return
			lastGestureRef.current = performance.now()
			if (contentRef.current) contentRef.current.style.willChange = ''
			const { zoom: startZoom, mid: startMid } = start
			start = null
			const nextZoom = clampZoom(startZoom * ratio)
			if (nextZoom !== startZoom) {
				// the spot under the starting midpoint ends under the final one
				zoomAnchorRef.current = anchorForPinch(startMid, mid)
				setPreferences({ zoom: nextZoom })
			} else if (contentRef.current) {
				contentRef.current.style.transform = ''
			}
			ratio = 1
		}
		/** The page spot under the pinch's first midpoint, to put under its last one */
		const anchorForPinch = (from: Point, to: Point): ZoomAnchor | null => {
			const content = contentRef.current
			const transform = content?.style.transform ?? ''
			if (content) content.style.transform = ''
			const anchor = anchorAtRef.current(from)
			if (content) content.style.transform = transform
			if (!anchor) return null
			const box = element.getBoundingClientRect()
			return { ...anchor, x: to.x - box.left, y: to.y - box.top }
		}
		element.addEventListener('touchstart', onTouchStart, { passive: true })
		element.addEventListener('touchmove', onTouchMove, { passive: false })
		element.addEventListener('touchend', onTouchEnd)
		element.addEventListener('touchcancel', onTouchEnd)
		return () => {
			element.removeEventListener('touchstart', onTouchStart)
			element.removeEventListener('touchmove', onTouchMove)
			element.removeEventListener('touchend', onTouchEnd)
			element.removeEventListener('touchcancel', onTouchEnd)
		}
	}, [mediaId, setPreferences])

	// A deliberate sideways swipe turns the page in the paged layout. The page isn't dragged
	// along with the finger (at the fit it can't move sideways at all): it simply turns once the
	// finger lifts. Zoomed in, a sideways drag pans the page instead, and a swipe that ends a
	// text selection (a long press, then a drag) selects rather than turns.
	const swipeStateRef = useRef({ paged: !continuous, zoomedIn, goRelative })
	useEffect(() => {
		swipeStateRef.current = { paged: !continuous, zoomedIn, goRelative }
	}, [continuous, zoomedIn, goRelative])
	useEffect(() => {
		const element = scrollRef.current
		if (!element) return undefined
		let start: { x: number; y: number; time: number } | null = null
		const onTouchStart = (event: TouchEvent) => {
			const touch = event.touches[0]
			// a second finger makes it a pinch, not a swipe
			start =
				event.touches.length === 1 && touch
					? { x: touch.clientX, y: touch.clientY, time: performance.now() }
					: null
		}
		const onTouchEnd = (event: TouchEvent) => {
			const touch = event.changedTouches[0]
			const from = start
			start = null
			if (!from || !touch || event.touches.length > 0) return
			const { paged, zoomedIn: zoomed, goRelative: turn } = swipeStateRef.current
			if (!paged || zoomed) return
			const selection = window.getSelection()
			if (selection && !selection.isCollapsed) return
			const direction = swipeTurn(
				swipeOf(
					touch.clientX - from.x,
					touch.clientY - from.y,
					performance.now() - from.time,
					element.clientWidth,
				),
			)
			if (!direction) return
			lastGestureRef.current = performance.now()
			turn(direction)
		}
		const onTouchCancel = () => {
			start = null
		}
		element.addEventListener('touchstart', onTouchStart, { passive: true })
		element.addEventListener('touchend', onTouchEnd)
		element.addEventListener('touchcancel', onTouchCancel)
		return () => {
			element.removeEventListener('touchstart', onTouchStart)
			element.removeEventListener('touchend', onTouchEnd)
			element.removeEventListener('touchcancel', onTouchCancel)
		}
	}, [])

	// Taps and clicks on the pages ------------------------------------------------------------------
	const handlePointerDown = useCallback((event: React.PointerEvent) => {
		pointerDownRef.current = true
		const layer = (event.target as Element).closest?.('.pdf-text-layer')
		layer?.classList.add('selecting')
	}, [])

	const handlePointerUp = useCallback(() => {
		pointerDownRef.current = false
		document
			.querySelectorAll('.pdf-text-layer.selecting')
			.forEach((layer) => layer.classList.remove('selecting'))
		if (isCoarsePointer) return // the settled selectionchange shows the toolbar
		const next = readSelectionTarget()
		if (next) setTarget(next)
	}, [readSelectionTarget, isCoarsePointer, setTarget])

	const lastTapRef = useRef<{ time: number; point: Point } | null>(null)
	const pendingTapRef = useRef<number | undefined>(undefined)
	useEffect(() => () => window.clearTimeout(pendingTapRef.current), [])

	const handleClick = useCallback(
		(event: React.MouseEvent) => {
			const selection = window.getSelection()
			if (selection && !selection.isCollapsed) return
			// the end of a pinch or a pan is not a tap
			if (performance.now() - lastGestureRef.current < GESTURE_QUIET_MS) return

			const pageElement = (event.target as Element).closest?.('[data-pdf-page]')
			if (pageElement) {
				const page = Number(pageElement.getAttribute('data-pdf-page'))
				const box = pageElement.getBoundingClientRect()
				const x = (event.clientX - box.left) / box.width
				const y = (event.clientY - box.top) / box.height
				const hit = [...(annotationsByPage.get(page) ?? [])]
					.reverse()
					.find((annotation) =>
						(annotation.locator.locations?.rects ?? []).some((rect) => rectContains(rect, x, y)),
					)
				if (hit) {
					openAnnotation(hit.id)
					return
				}
			}

			if (target) {
				setTarget(null)
				return
			}

			const container = scrollRef.current
			if (!container) return
			const box = container.getBoundingClientRect()
			const relative = (event.clientX - box.left) / box.width
			const point = { x: event.clientX, y: event.clientY }
			// the same side zones as the other readers (a third of the width at each side on a
			// touch screen, a quarter with a mouse); in the scrolled layout the sides don't turn
			// anything, a tap only shows the controls
			const zone = navigationTapZone(relative, {
				flow: continuous ? 'continuous' : 'paged',
				coarse: isCoarsePointer,
			})
			if (zone !== 'middle') {
				goRelative(zoneTurn(zone))
				return
			}
			if (!isCoarsePointer) return
			// A page with selectable text: a tap shows or hides the top bar right away. No double
			// tap to zoom there: on Android a double tap is also how text gets selected (pinch zooms)
			if (layout.hasText) {
				chrome.toggle()
				return
			}
			// A scan (no text): a double tap zooms to 2x around the tap (or back to the fit); a
			// single tap shows or hides the top bar once it's clear no second tap follows
			const now = performance.now()
			if (isDoubleTap(lastTapRef.current, now, point)) {
				window.clearTimeout(pendingTapRef.current)
				lastTapRef.current = null
				zoomAround(zoom > 1.05 ? 1 : 2, point)
				return
			}
			lastTapRef.current = { time: now, point }
			window.clearTimeout(pendingTapRef.current)
			pendingTapRef.current = window.setTimeout(() => chrome.toggle(), DOUBLE_TAP_MS)
		},
		[
			annotationsByPage,
			openAnnotation,
			target,
			continuous,
			goRelative,
			isCoarsePointer,
			zoomAround,
			zoom,
			chrome,
			setTarget,
			layout.hasText,
		],
	)

	// Render ------------------------------------------------------------------------------------------
	const range: [number, number] | null = continuous
		? viewport.height > 0
			? mountRange(scrollLayout, scrollTop, viewport.height)
			: [startIndex, startIndex]
		: visible

	const renderPage = (index: number) => {
		const size = sizes[index]
		const pageSize = layout.pages[index]
		if (!size || !pageSize || size.width <= 0) return null
		const page = index + 1
		return (
			<PdfPageView
				key={page}
				mediaId={mediaId}
				page={page}
				v={layout.v}
				size={size}
				aspect={pageSize[0] / pageSize[1]}
				src={srcFor(index)}
				canLoad={visibleLoaded || (!!visible && index >= visible[0] && index <= visible[1])}
				onPictureLoaded={handlePictureLoaded}
				hasText={layout.hasText}
				annotations={annotationsByPage.get(page) ?? NO_ANNOTATIONS}
				look={look}
				selectedAnnotationId={target?.kind === 'annotation' ? target.annotationId : null}
				flashRects={flash?.page === page ? flash.rects : null}
				onOpenAnnotation={openAnnotation}
				top={pageTop(index)}
				left={pageLeft(index)}
			/>
		)
	}

	const pages: React.ReactNode[] = []
	if (range) {
		for (let index = range[0]; index <= range[1]; index++) {
			pages.push(renderPage(index))
		}
	}

	const shortcutRows = useMemo<ShortcutRow[]>(
		() => [
			{ keys: ['←', '→'], label: translate('shared.readerAnnotations.shortcuts.turnPage') },
			{ keys: ['PgUp', 'PgDn'], label: translate('shared.reader.shortcuts.turnPageAlways') },
			{ keys: ['←', '↑', '↓', '→'], label: translate('shared.reader.shortcuts.panZoomed') },
			{ keys: ['Space'], label: translate('shared.pdfReader.shortcuts.screen') },
			{ keys: ['+', '-', '0'], label: translate('shared.pdfReader.shortcuts.zoom') },
			{ keys: ['F'], label: translate('shared.pdfReader.shortcuts.fit') },
			{ keys: ['G'], label: translate('shared.pdfReader.goToPage') },
		],
		[translate],
	)

	const pagedContentHeight = !continuous
		? Math.max(viewport.height, (scrollLayout.rows[pagedRow]?.height ?? 0) + gap * 2)
		: 0
	const label = pagesInView.length === 1 && layout.labels ? labelOf(currentPage) : null
	const indicator = `${label && label !== String(currentPage) ? `${label} · ` : ''}${pageIndicatorText(pagesInView, pageCount)}`

	const layoutDisabledReason = useCallback(
		(option: ReaderLayout) =>
			option.spread === 'double' && viewport.width > 0 && !canShowTwoPages(viewport.width)
				? translate('shared.reader.layout.needsWiderScreen')
				: null,
		[viewport.width, translate],
	)

	return (
		<div
			className="min-h-0 relative h-dvh w-full overflow-hidden"
			style={{ backgroundColor: look.background }}
			data-testid="pdf-reader"
		>
			<style>{PDF_READER_CSS}</style>

			<div
				ref={scrollRef}
				// no scrollbar beside the pages (scrolling still works): a reader shows the page
				// number instead of a bar at the edge
				className="absolute overflow-auto [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
				style={{
					// clear of a camera cutout or rounded corners: the pages never lie under them
					// (the strips around keep the reader's ground colour)
					top: 'env(safe-area-inset-top)',
					bottom: 'env(safe-area-inset-bottom)',
					left: 'env(safe-area-inset-left)',
					right: 'env(safe-area-inset-right)',
					overflowAnchor: 'none',
					// at the fit, only vertical scrolling: no sideways wobble, no stretch at the ends
					touchAction: touchStyle.touchAction,
					overflowX: touchStyle.overflowX,
					overscrollBehavior: 'none',
				}}
				onScroll={handleScroll}
				onPointerDown={handlePointerDown}
				onPointerUp={handlePointerUp}
				onPointerCancel={handlePointerUp}
				onClick={handleClick}
				data-testid="pdf-scroll"
			>
				<div
					ref={contentRef}
					className="relative"
					style={{
						width: contentWidth,
						height: continuous ? scrollLayout.totalHeight : pagedContentHeight,
					}}
				>
					{pages}
				</div>
			</div>

			<ReaderTopBar
				chrome={chrome}
				title={media.resolvedName}
				backTo={paths.bookOverview(mediaId)}
				onContents={() => setLocationsOpen(true)}
				onSearch={layout.hasText ? () => setSearchOpen(true) : undefined}
				bookmark={{ active: !!currentBookmark, onToggle: () => void toggleBookmark() }}
				onSettings={() => openSettings()}
			/>

			<ReaderPageIndicator
				text={indicator}
				onClick={() => setGoToOpen(true)}
				title={`${translate('shared.pdfReader.goToPage')} (G)`}
			/>

			{target && (
				<AnnotationToolbar
					key={
						target.kind === 'annotation'
							? target.annotationId
							: `selection-${target.selection.locator.locations?.fragments?.join()}`
					}
					anchor={target.kind === 'selection' ? target.selection.rect : target.rect}
					mode={target.kind}
					activeStyle={targetAnnotation?.style}
					hasNote={!!targetAnnotation?.annotationText}
					onStyle={applyStyle}
					onNote={openNote}
					onCopy={() => void copyTarget()}
					onDelete={target.kind === 'annotation' ? removeTargetAnnotation : undefined}
				/>
			)}

			{noteDialog && (
				<AnnotationNoteDialog
					open
					mode={noteDialog.mode}
					quotedText={
						noteDialog.mode === 'create'
							? noteDialog.quotedText
							: (noteDialog.annotation.locator.text?.highlight ?? undefined)
					}
					initialNote={
						noteDialog.mode === 'edit' ? (noteDialog.annotation.annotationText ?? '') : ''
					}
					initialStyle={noteDialog.mode === 'edit' ? noteDialog.annotation.style : noteDialog.style}
					onOpenChange={(open) => !open && setNoteDialog(null)}
					onSave={(note, style) => {
						if (noteDialog.mode === 'create') {
							createAnnotation(noteDialog.locator, { style, annotationText: note })
							clearSelection()
						} else {
							updateAnnotation(noteDialog.annotation.id, { style, annotationText: note || null })
						}
						setNoteDialog(null)
					}}
					onDelete={
						noteDialog.mode === 'edit'
							? () => {
									deleteAnnotation(noteDialog.annotation.id)
									setNoteDialog(null)
									setTarget(null)
								}
							: undefined
					}
				/>
			)}

			<ReaderSettingsDialog
				open={settingsOpen}
				onOpenChange={setSettingsOpen}
				section={settingsSection}
				layout={storedLayout}
				onLayoutChange={(next) =>
					setPreferences({
						spread: next.spread,
						scrollMode: next.flow === 'paged' ? 'paged' : 'continuous',
					})
				}
				layoutDisabledReason={layoutDisabledReason}
				more={
					storedLayout.spread === 'double' ? (
						<SwitchRow
							label={translate('shared.reader.layout.coverAlone')}
							hint={translate('shared.reader.layout.coverAloneHint')}
							checked={coverAlone}
							onCheckedChange={(checked) => setPreferences({ coverAlone: checked })}
						/>
					) : null
				}
				shortcuts={shortcutRows}
				footer={<PdfOpenAs bookId={mediaId} onChange={setPreferences} />}
			>
				<PdfPageSettings
					fit={fit}
					zoom={zoom}
					invertPagesInDark={invertPagesInDark}
					onChange={setPreferences}
				/>
			</ReaderSettingsDialog>

			<PdfSearchDialog
				mediaId={mediaId}
				open={searchOpen}
				onOpenChange={setSearchOpen}
				labelOf={labelOf}
				onSelect={showSearchResult}
			/>
			<PdfGoToPageDialog
				open={goToOpen}
				onOpenChange={setGoToOpen}
				pageCount={pageCount}
				resolve={(input) => resolvePageInput(layout, input)}
				onGo={(page) => goToPage(page)}
			/>
			<PdfLocationsDialog
				open={locationsOpen}
				onOpenChange={setLocationsOpen}
				tab={locationsTab}
				onTabChange={setLocationsTab}
				outline={layout.outline}
				bookmarks={bookmarks}
				annotations={annotations}
				hasText={layout.hasText}
				labelOf={labelOf}
				onGoToPage={goToPage}
				onDeleteBookmark={deleteBookmark}
			/>
		</div>
	)
}

const NO_ANNOTATIONS: ReaderAnnotation[] = []
