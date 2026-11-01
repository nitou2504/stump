import { useSDK } from '@stump/client'
import { cn, FullScreenLoader, Spacer, Text } from '@stump/components'
import type { BookReaderSceneQuery } from '@stump/graphql'
import type { PdfLayout, PdfSearchResult } from '@stump/sdk'
import { useQueryClient } from '@tanstack/react-query'
import { ArrowLeft, Bookmark, Fullscreen, Keyboard, List, Search, Shrink } from 'lucide-react'
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import { Link } from 'react-router'
import { useFullscreen } from 'rooks'
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
	ReaderShortcutsHelp,
	type ShortcutRow,
	type ToolbarAnchor,
} from '../annotations'
import ControlButton from '../epub/controls/ControlButton'
import DayNightToggle from '../epub/controls/DayNightToggle'
import { useReaderChromeTheme } from '../epub/readium/useReaderChromeTheme'
import {
	boundingRect,
	clampZoom,
	type FitMode,
	type FractionRect,
	pageCssSize,
	pickTier,
	rectContains,
	zoomIn,
	zoomOut,
} from './geometry'
import { chapterTitleForPage, flattenOutline, pageLabel, resolvePageInput } from './outline'
import PdfAppearanceDialog from './PdfAppearanceDialog'
import PdfGoToPageDialog from './PdfGoToPageDialog'
import PdfLocationsDialog, { type PdfLocationTab } from './PdfLocationsDialog'
import PdfPageView from './PdfPageView'
import { PDF_READER_CSS } from './pdfReaderCss'
import PdfSearchDialog from './PdfSearchDialog'
import { resolvePdfShortcut } from './pdfShortcuts'
import { pdfThemeLook, resolvePdfTheme } from './pdfThemes'
import {
	buildScrollLayout,
	dominantPage,
	mountRange,
	prefetchIndexes,
	progressionWithin,
	scrollTopFor,
} from './scrollLayout'
import { readPdfSelection, selectionToLocator, totalProgression } from './selection'
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
	const { data: layout, isLoading, error, refetch } = usePdfLayout(props.media.id)
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
	const rootRef = useRef<HTMLDivElement>(null!)
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
	const scrollMode = preferences.scrollMode ?? 'continuous'
	const continuous = scrollMode === 'continuous'
	const gap = isPhone ? PAGE_GAP_PHONE : PAGE_GAP_DESKTOP
	const sidePadding = isPhone ? 0 : PAGE_GAP_DESKTOP

	const available = useMemo(
		() => ({
			width: Math.max(0, viewport.width - sidePadding * 2),
			height: Math.max(0, viewport.height - gap * 2),
		}),
		[viewport.width, viewport.height, sidePadding, gap],
	)
	const sizes = useMemo(
		() => layout.pages.map((page) => pageCssSize(page, available, fit, zoom)),
		[layout.pages, available, fit, zoom],
	)
	const scrollLayout = useMemo(() => buildScrollLayout(sizes, gap), [sizes, gap])
	const contentWidth = useMemo(
		() => Math.max(viewport.width, ...sizes.map((size) => size.width + sidePadding * 2)),
		[sizes, viewport.width, sidePadding],
	)
	const dpr = typeof window !== 'undefined' ? window.devicePixelRatio || 1 : 1

	// Position ------------------------------------------------------------------------------
	const startIndex = useMemo(() => {
		const page = initialPage ?? media.readProgress?.page ?? 1
		return Math.min(pageCount, Math.max(1, page || 1)) - 1
	}, [initialPage, media.readProgress?.page, pageCount])

	const [scrollTop, setScrollTop] = useState(0)
	const [pagedIndex, setPagedIndex] = useState(startIndex)
	/** The reading position kept across zoom, fit and resize changes */
	const anchorRef = useRef({ index: startIndex, progression: 0 })
	const programmaticScrollRef = useRef(false)

	const currentIndex = continuous
		? viewport.height > 0
			? dominantPage(scrollLayout, scrollTop, viewport.height)
			: startIndex
		: pagedIndex
	const currentPage = currentIndex + 1

	// Restore the anchor whenever the layout changes (first paint, zoom, fit, resize, mode)
	useLayoutEffect(() => {
		const element = scrollRef.current
		if (!element || viewport.height <= 0) return
		if (continuous) {
			const { index, progression } = anchorRef.current
			programmaticScrollRef.current = true
			element.scrollTop = scrollTopFor(scrollLayout, index, progression)
			setScrollTop(element.scrollTop)
		} else {
			setPagedIndex(anchorRef.current.index)
		}
	}, [scrollLayout, continuous, viewport.height])

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
			const slot = scrollLayout.slots[index]
			anchorRef.current = {
				index,
				progression: slot && top > slot.top ? progressionWithin(scrollLayout, index, top) : 0,
			}
		})
	}, [continuous, scrollLayout])

	useEffect(
		() => () => {
			if (frameRef.current != null) window.cancelAnimationFrame(frameRef.current)
		},
		[],
	)

	const goToPage = useCallback(
		(page: number, progression = 0) => {
			const index = Math.min(pageCount - 1, Math.max(0, Math.round(page) - 1))
			// keep a little context above a target inside the page
			const within = progression > 0 ? Math.max(0, progression - 0.04) : 0
			anchorRef.current = { index, progression: within }
			if (continuous) {
				const element = scrollRef.current
				if (!element) return
				programmaticScrollRef.current = true
				element.scrollTop = scrollTopFor(scrollLayout, index, within)
				setScrollTop(element.scrollTop)
			} else {
				setPagedIndex(index)
				scrollRef.current?.scrollTo({ top: 0 })
			}
		},
		[continuous, pageCount, scrollLayout],
	)

	const goRelative = useCallback(
		(delta: number) => goToPage(currentIndex + 1 + delta),
		[currentIndex, goToPage],
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

	const prefetchText = usePrefetchPdfText(mediaId, layout.v)
	useEffect(() => {
		if (viewport.width <= 0) return undefined
		const indexes = prefetchIndexes(currentIndex, pageCount)
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
		pageCount,
		tierFor,
		sdk,
		mediaId,
		layout.v,
		layout.hasText,
		prefetchText,
		viewport.width,
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
	}, [])

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
		[annotations, annotationAnchor],
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

	const applyStyle = useCallback(
		(style: ReaderAnnotation['style']) => {
			if (!target) return
			if (target.kind === 'selection') {
				createAnnotation(target.selection.locator, { style })
				clearSelection()
			} else {
				updateAnnotation(target.annotationId, { style })
			}
		},
		[target, createAnnotation, updateAnnotation, clearSelection],
	)

	const openNote = useCallback(() => {
		if (!target) return
		if (target.kind === 'selection') {
			setNoteDialog({
				mode: 'create',
				locator: target.selection.locator,
				quotedText: target.selection.text,
				style: 'HIGHLIGHT' as ReaderAnnotation['style'],
			})
		} else if (targetAnnotation) {
			setNoteDialog({ mode: 'edit', annotation: targetAnnotation })
		}
	}, [target, targetAnnotation])

	const removeTargetAnnotation = useCallback(() => {
		if (target?.kind !== 'annotation') return
		deleteAnnotation(target.annotationId)
		setTarget(null)
	}, [target, deleteAnnotation])

	// Dialogs and chrome --------------------------------------------------------------------------
	const [helpOpen, setHelpOpen] = useState(false)
	const [searchOpen, setSearchOpen] = useState(false)
	const [goToOpen, setGoToOpen] = useState(false)
	const [locationsOpen, setLocationsOpen] = useState(false)
	const [locationsTab, setLocationsTab] = useState<PdfLocationTab>('contents')
	const [chromeVisible, setChromeVisible] = useState(true)
	const [flash, setFlash] = useState<Flash | null>(null)

	const { isFullscreenEnabled, toggleFullscreen } = useFullscreen({ target: rootRef })

	const showSearchResult = useCallback(
		(result: PdfSearchResult) => {
			setSearchOpen(false)
			goToPage(result.page, result.rects[0]?.y ?? 0)
			setFlash({ page: result.page, rects: result.rects, key: Date.now() })
		},
		[goToPage],
	)

	useEffect(() => {
		if (!flash) return undefined
		const timeout = window.setTimeout(() => setFlash(null), 2600)
		return () => window.clearTimeout(timeout)
	}, [flash])

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
				case 'delete':
					removeTargetAnnotation()
					break
				case 'bookmark':
					void toggleBookmark()
					break
				case 'help':
					setHelpOpen(true)
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
				case 'search':
					setSearchOpen(true)
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
		removeTargetAnnotation,
		toggleBookmark,
		clearSelection,
		goRelative,
		scrollByScreen,
		goToPage,
		pageCount,
		setPreferences,
		zoom,
		fit,
	])

	// Ctrl/Cmd + wheel zooms the pages instead of the whole app
	useEffect(() => {
		const element = scrollRef.current
		if (!element) return undefined
		let pending = 1
		let timeout: number | undefined
		const onWheel = (event: WheelEvent) => {
			if (!event.ctrlKey && !event.metaKey) return
			event.preventDefault()
			pending *= Math.exp(-event.deltaY / 400)
			window.clearTimeout(timeout)
			timeout = window.setTimeout(() => {
				const current = clampZoom(usePdfReaderStore.getState().books[mediaId]?.zoom ?? 1)
				setPreferences({ zoom: clampZoom(current * pending) })
				pending = 1
			}, 120)
		}
		element.addEventListener('wheel', onWheel, { passive: false })
		return () => {
			window.clearTimeout(timeout)
			element.removeEventListener('wheel', onWheel)
		}
	}, [mediaId, setPreferences])

	// Pinch to zoom on touch screens (two fingers); applied when the fingers lift
	useEffect(() => {
		const element = scrollRef.current
		if (!element) return undefined
		let startDistance = 0
		let ratio = 1
		const distance = (touches: TouchList) =>
			Math.hypot(
				touches[0]!.clientX - touches[1]!.clientX,
				touches[0]!.clientY - touches[1]!.clientY,
			)
		const onTouchStart = (event: TouchEvent) => {
			if (event.touches.length === 2) {
				startDistance = distance(event.touches)
				ratio = 1
			}
		}
		const onTouchMove = (event: TouchEvent) => {
			if (event.touches.length !== 2 || !startDistance) return
			event.preventDefault()
			ratio = distance(event.touches) / startDistance
			if (contentRef.current) contentRef.current.style.transform = `scale(${ratio})`
		}
		const onTouchEnd = (event: TouchEvent) => {
			if (!startDistance || event.touches.length > 0) return
			if (contentRef.current) contentRef.current.style.transform = ''
			if (Math.abs(ratio - 1) > 0.05) {
				const current = clampZoom(usePdfReaderStore.getState().books[mediaId]?.zoom ?? 1)
				setPreferences({ zoom: clampZoom(current * ratio) })
			}
			startDistance = 0
			ratio = 1
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
		const next = readSelectionTarget()
		if (next) setTarget(next)
	}, [readSelectionTarget])

	const handleClick = useCallback(
		(event: React.MouseEvent) => {
			const selection = window.getSelection()
			if (selection && !selection.isCollapsed) return

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
			if (!continuous && relative < 0.25) {
				goRelative(-1)
			} else if (!continuous && relative > 0.75) {
				goRelative(1)
			} else if (isCoarsePointer) {
				setChromeVisible((visible) => !visible)
			}
		},
		[annotationsByPage, openAnnotation, target, continuous, goRelative, isCoarsePointer],
	)

	// Render ------------------------------------------------------------------------------------------
	const range: [number, number] | null = continuous
		? viewport.height > 0
			? mountRange(scrollLayout, scrollTop, viewport.height)
			: [startIndex, startIndex]
		: [pagedIndex, pagedIndex]

	const renderPage = (index: number, style: React.CSSProperties) => {
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
				hasText={layout.hasText}
				annotations={annotationsByPage.get(page) ?? NO_ANNOTATIONS}
				look={look}
				selectedAnnotationId={target?.kind === 'annotation' ? target.annotationId : null}
				flashRects={flash?.page === page ? flash.rects : null}
				onOpenAnnotation={openAnnotation}
				style={style}
			/>
		)
	}

	const pages: React.ReactNode[] = []
	if (range) {
		for (let index = range[0]; index <= range[1]; index++) {
			if (continuous) {
				const slot = scrollLayout.slots[index]
				if (!slot) continue
				pages.push(
					renderPage(index, {
						top: slot.top,
						left: Math.max(sidePadding, (contentWidth - slot.width) / 2),
					}),
				)
			} else {
				const size = sizes[index]
				if (!size) continue
				pages.push(
					renderPage(index, {
						top: Math.max(gap, (viewport.height - size.height) / 2),
						left: Math.max(sidePadding, (contentWidth - size.width) / 2),
					}),
				)
			}
		}
	}

	const shortcutRows = useMemo<ShortcutRow[]>(
		() => [
			{ keys: ['←', '→'], label: translate('shared.readerAnnotations.shortcuts.turnPage') },
			{ keys: ['Space'], label: translate('shared.pdfReader.shortcuts.screen') },
			{ keys: ['+', '-', '0'], label: translate('shared.pdfReader.shortcuts.zoom') },
			{ keys: ['F'], label: translate('shared.pdfReader.shortcuts.fit') },
			{ keys: ['G'], label: translate('shared.pdfReader.goToPage') },
			{ keys: ['Ctrl', 'K'], label: translate('shared.readerAnnotations.shortcuts.search') },
		],
		[translate],
	)

	const pagedContentHeight = !continuous
		? Math.max(viewport.height, (sizes[pagedIndex]?.height ?? 0) + gap * 2)
		: 0
	const progressPercent = Math.round((currentPage / Math.max(1, pageCount)) * 100)

	return (
		<div
			ref={rootRef}
			className="min-h-0 flex h-dvh w-full flex-col overflow-hidden"
			style={{ backgroundColor: look.background }}
			data-testid="pdf-reader"
		>
			<style>{PDF_READER_CSS}</style>

			{chromeVisible && (
				<header className="h-11 gap-1 px-2 flex shrink-0 items-center bg-background text-foreground">
					<Link
						to={paths.bookOverview(mediaId)}
						title={translate('shared.imageReader.header.goToMediaOverview')}
						aria-label={translate('shared.imageReader.header.goToMediaOverview')}
					>
						<ControlButton tabIndex={-1}>
							<ArrowLeft className="h-4 w-4" />
						</ControlButton>
					</Link>
					<ControlButton
						title={translate('shared.epubReader.locationManager')}
						aria-label={translate('shared.epubReader.locationManager')}
						onClick={() => setLocationsOpen(true)}
					>
						<List className="h-4 w-4" />
					</ControlButton>
					<Spacer />
					<Text size="sm" className="px-2 min-w-0 line-clamp-1">
						{media.resolvedName}
					</Text>
					<Spacer />
					{layout.hasText && (
						<ControlButton
							title={`${translate('shared.readerAnnotations.shortcuts.search')} (Ctrl+K)`}
							aria-label={translate('shared.readerAnnotations.shortcuts.search')}
							onClick={() => setSearchOpen(true)}
						>
							<Search className="h-4 w-4" />
						</ControlButton>
					)}
					<DayNightToggle />
					<PdfAppearanceDialog
						bookId={mediaId}
						fit={fit}
						zoom={zoom}
						scrollMode={scrollMode}
						invertPagesInDark={invertPagesInDark}
						onChange={setPreferences}
					/>
					<span className="md:contents hidden">
						<ControlButton
							title={`${translate('shared.readerAnnotations.shortcuts.title')} (?)`}
							aria-label={translate('shared.readerAnnotations.shortcuts.title')}
							onClick={() => setHelpOpen(true)}
						>
							<Keyboard className="h-4 w-4" />
						</ControlButton>
						<ControlButton
							title={translate('shared.pdfReader.fullscreen')}
							aria-label={translate('shared.pdfReader.fullscreen')}
							aria-pressed={isFullscreenEnabled}
							onClick={() => void toggleFullscreen()}
						>
							{isFullscreenEnabled ? (
								<Shrink className="h-4 w-4" />
							) : (
								<Fullscreen className="h-4 w-4" />
							)}
						</ControlButton>
					</span>
					<ControlButton
						title={`${translate(
							currentBookmark
								? 'shared.readerAnnotations.removeBookmark'
								: 'shared.readerAnnotations.addBookmark',
						)} (B)`}
						aria-label={translate(
							currentBookmark
								? 'shared.readerAnnotations.removeBookmark'
								: 'shared.readerAnnotations.addBookmark',
						)}
						aria-pressed={!!currentBookmark}
						onClick={() => void toggleBookmark()}
					>
						<Bookmark className={cn('h-4 w-4', { 'fill-current': !!currentBookmark })} />
					</ControlButton>
				</header>
			)}

			<div
				ref={scrollRef}
				className="min-h-0 relative w-full flex-1 overflow-auto"
				style={{ overflowAnchor: 'none', touchAction: 'pan-x pan-y' }}
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
						transformOrigin: 'center top',
					}}
				>
					{pages}
				</div>
			</div>

			{chromeVisible && (
				<footer className="h-9 gap-3 px-3 flex shrink-0 items-center bg-background text-foreground">
					<button
						type="button"
						className="text-xs rounded px-1 shrink-0 text-muted-foreground tabular-nums hover:text-foreground"
						title={`${translate('shared.pdfReader.goToPage')} (G)`}
						onClick={() => setGoToOpen(true)}
						data-testid="pdf-page-indicator"
					>
						{layout.labels ? `${labelOf(currentPage)} · ` : ''}
						{currentPage} / {pageCount}
					</button>
					<div className="h-1.5 relative flex-1 overflow-hidden rounded-full bg-muted-foreground/30">
						<div
							className="inset-y-0 left-0 absolute bg-muted-foreground"
							style={{ width: `${progressPercent}%` }}
						/>
					</div>
					{!layout.hasText && (
						<Text size="xs" variant="muted" className="sm:block hidden shrink-0">
							{translate('shared.pdfReader.noTextShort')}
						</Text>
					)}
				</footer>
			)}

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
			<ReaderShortcutsHelp open={helpOpen} onOpenChange={setHelpOpen} extra={shortcutRows} />
		</div>
	)
}

const NO_ANNOTATIONS: ReaderAnnotation[] = []
