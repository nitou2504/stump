import type { OnDecorationActivatedEvent } from '@readium/navigator'
import type { BasicTextSelection } from '@readium/navigator-html-injectables'
import { Layout, Link, Locator } from '@readium/shared'
import {
	parseGraphQLDecimal,
	queryClient,
	useGraphQLMutation,
	useSDK,
	useSuspenseGraphQL,
} from '@stump/client'
import {
	AnnotationStyle,
	Bookmark,
	EpubProgressInput,
	graphql,
	ReadingDirection,
	ReadingMode,
	type ReadiumLocator,
} from '@stump/graphql'
import type { EpubSearchResponse } from '@stump/sdk'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useDebounce } from 'rooks'
import { toast } from 'sonner'

import Spinner from '@/components/Spinner'
import { useTheme } from '@/hooks'
import { useTranslate } from '@/hooks/useTranslate'
import { useBookPreferences } from '@/scenes/book/reader/useBookPreferences'
import { useReaderStore } from '@/stores'
import { useBookTimer } from '@/stores/reader'

import {
	AnnotationNoteDialog,
	type AnnotationSelection,
	type AnnotationTarget,
	AnnotationToolbar,
	graphqlAnnotationToReaderAnnotation,
	isEditableTarget,
	isOptimisticAnnotationId,
} from '../../annotations'
import { copyText } from '../../shared/readerKeys'
import { READING_BAR_ZONE } from '../../shared/ReadingBar'
import {
	type ChapterPages,
	chapterPages,
	tocPath,
	useReadingProgress,
} from '../../shared/readingProgress'
import { annotationsToDecorations, type DecorationTheme } from '../annotations/decorations'
import { useEdgeHoldExtend } from '../annotations/edgeExtend'
import { liveFrameSelection } from '../annotations/liveSelection'
import {
	decorationEventViewportRect,
	enrichSelectionLocator,
	extractSelectionContext,
	selectionViewportRect,
} from '../annotations/locator'
import type { EpubAnnotation } from '../annotations/types'
import { ANNOTATION_DECORATION_GROUP } from '../annotations/types'
import { useEpubAnnotations } from '../annotations/useEpubAnnotations'
import { EpubContent, type ReaderLocator } from '../context'
import EpubReaderShortcuts, { type AnnotationShortcutAction } from '../controls/EpubReaderShortcuts'
import { READER_PAGE_TAP_EVENT, type ReaderPageTapDetail } from '../controls/pageTap'
import EpubReaderContainer from '../EpubReaderContainer'
import {
	extractDocumentBody,
	extractNote,
	findTargetElement,
	parseBookPage,
} from '../links/noteContent'
import NotePopup, { type OpenNote } from '../links/NotePopup'
import {
	isExternalHref,
	isNoteLink,
	isShortReference,
	type LinkFacts,
	linkLooksLikeNote,
	resolvePublicationTarget,
	targetLocator,
} from '../links/resolveLink'
import ReturnChip from '../links/ReturnChip'
import { useLinkReturnStack } from '../links/useLinkReturnStack'
import { createStumpFetch } from './createStumpHttpFetcher'
import {
	hrefsMatch,
	packagePathFromHref,
	resolveInitialLocator,
	toolkitLocatorToInput,
	toolkitLocatorToReaderLocator,
	unmangleHref,
} from './locator'
import { type OpenedPublication, openStumpPublication } from './openPublication'
import { bookPreferencesToEpubPreferences, resolveReaderTheme } from './preferences'
import { useReaderChromeTheme } from './useReaderChromeTheme'
import {
	type ReaderLinkActivation,
	type ReadiumNavigatorApi,
	useReadiumNavigator,
} from './useReadiumNavigator'

type Props = {
	id: string
	isIncognito: boolean
}

const query = graphql(`
	query ReadiumWebReader($id: ID!) {
		epubById(id: $id) {
			mediaId
			toc
			bookmarks {
				id
				userId
				mediaId
				previewContent
				createdAt
				locator {
					chapterTitle
					href
					title
					type
					locations {
						fragments
						progression
						position
						totalProgression
						cssSelector
					}
					text {
						after
						before
						highlight
					}
				}
			}
			annotations {
				id
				mediaId
				userId
				annotationText
				style
				createdAt
				updatedAt
				locator {
					chapterTitle
					href
					title
					type
					locations {
						fragments
						progression
						position
						totalProgression
						cssSelector
					}
					text {
						after
						before
						highlight
					}
				}
			}
			media {
				id
				resolvedName
				pages
				extension
				readProgress {
					percentageCompleted
					page
					elapsedSeconds
					locator {
						chapterTitle
						href
						title
						type
						locations {
							fragments
							progression
							position
							totalProgression
							cssSelector
						}
						text {
							after
							before
							highlight
						}
					}
				}
				libraryConfig {
					defaultReadingImageScaleFit
					defaultReadingMode
					defaultReadingDir
				}
				nextInSeries(pagination: { cursor: { limit: 1 } }) {
					nodes {
						id
						name: resolvedName
						thumbnail {
							url
						}
					}
				}
			}
		}
	}
`)

const mutation = graphql(`
	mutation UpdateReadiumEpubProgress($id: ID!, $input: MediaProgressInput!) {
		updateMediaProgress(id: $id, input: $input) {
			__typename
		}
	}
`)

// Stable fallbacks: fresh `[]`s here changed the navigator effect's dependencies on every
// render while a publication was (re)opening, and its idle `setLoadState` then looped
// (React error #185, e.g. when going back into the reader)
const NO_POSITIONS: Locator[] = []
const NO_DOMAINS: string[] = []
/** What is known about a link Readium handed over: nothing but its URL */
const NO_LINK_FACTS: LinkFacts = { text: '', epubTypes: [], roles: [], superscript: false }

/** A note shown in the popup, and where its own place in the book is (if the reader can go) */
type ShownNote = OpenNote & { place: Locator | null }

type LoadState =
	| { status: 'loading' }
	| { status: 'ready'; opened: OpenedPublication; initialLocator?: Locator }
	| { status: 'error'; message: string }

/**
 * Production Readium Web EPUB reader — streams via Stump RWPM.
 */
export default function ReadiumWebReader({ id, isIncognito }: Props) {
	const { t, translate } = useTranslate()
	const { sdk } = useSDK()
	const { isDarkVariant } = useTheme()
	const containerRef = useRef<HTMLDivElement>(null)
	const viewportRef = useRef<HTMLDivElement>(null)
	const [stageWidth, setStageWidth] = useState<number | undefined>(undefined)

	const {
		data: { epubById: ebook },
	} = useSuspenseGraphQL(query, ['readiumWebReader', id], {
		id: id || '',
	})

	const {
		bookPreferences: {
			fontSize,
			lineHeight,
			fontFamily,
			readingMode,
			readingDirection,
			trackElapsedTime,
			columnCount,
			pageMargins,
		},
	} = useBookPreferences({ book: ebook.media })
	// Theme and alignment are global reader settings, not per-book overrides
	const readerTheme = useReaderStore((state) => state.settings.theme)
	const textAlign = useReaderStore((state) => state.settings.textAlign)
	const themeColors = resolveReaderTheme(readerTheme, isDarkVariant)
	const decorationTheme = useMemo<DecorationTheme>(
		() => ({ text: themeColors.text, background: themeColors.background }),
		[themeColors.text, themeColors.background],
	)
	useReaderChromeTheme(readerTheme)
	const readingBar = useReadingProgress('epub')
	const reserveReadingBar = readingBar.display === 'bar' && readingBar.show === 'always'

	// Measures the viewport parent — i.e. the parent of the element Readium itself treats
	// as its viewport (`containerRef`'s parent, see `useReadiumNavigator`). Keeping the
	// measurement one level up means it is unaffected by the centering max-width we apply to
	// that inner element below, so there is no feedback loop between measured width and layout.
	useEffect(() => {
		const el = viewportRef.current
		if (!el) return undefined

		setStageWidth(el.clientWidth || undefined)

		const observer = new ResizeObserver((entries) => {
			const entry = entries[0]
			if (entry) {
				setStageWidth(entry.contentRect.width)
			}
		})
		observer.observe(el)
		return () => observer.disconnect()
	}, [])

	const timer = useBookTimer(ebook.media?.id || '', {
		initial: ebook.media?.readProgress?.elapsedSeconds,
		enabled: trackElapsedTime,
	})

	const lastSyncedElapsedRef = useRef(ebook.media?.readProgress?.elapsedSeconds ?? 0)
	const lastSyncedLocatorRef = useRef<ReadiumLocator | null>(
		ebook.media?.readProgress?.locator ?? null,
	)
	const hasReachedEndRef = useRef(false)

	const [openState, setOpenState] = useState<LoadState>({ status: 'loading' })
	const [currentLocator, setCurrentLocator] = useState<Locator | null>(null)
	const [localProgress, setLocalProgress] = useState<number | null>(
		parseGraphQLDecimal(ebook.media?.readProgress?.percentageCompleted),
	)

	// Intentionally does not invalidate `['readiumWebReader', id]` on every success — progress
	// mutations fire on (debounced) every position change, and refetching the reader's own
	// suspense query that often would cause avoidable re-renders / flicker while reading.
	const { mutate } = useGraphQLMutation(mutation, {
		onSuccess: () => {
			lastSyncedElapsedRef.current = timer.getCurrentTime()
		},
	})

	const preferences = useMemo(
		() =>
			bookPreferencesToEpubPreferences({
				fontSize,
				lineHeight,
				fontFamily,
				readingMode,
				isDarkVariant,
				columnCount,
				pageMargins,
				stageWidth,
				theme: readerTheme,
				textAlign,
			}),
		[
			readerTheme,
			textAlign,
			fontSize,
			lineHeight,
			fontFamily,
			readingMode,
			isDarkVariant,
			columnCount,
			pageMargins,
			stageWidth,
		],
	)

	// Open the publication once per book id
	useEffect(() => {
		const abort = new AbortController()
		setOpenState({ status: 'loading' })

		const open = async () => {
			try {
				const opened = await openStumpPublication(sdk, id, abort.signal)
				if (abort.signal.aborted) return

				const percentageCompleted = parseGraphQLDecimal(
					ebook.media?.readProgress?.percentageCompleted,
				)

				const initialLocator = isIncognito
					? undefined
					: resolveInitialLocator({
							positions: opened.positions,
							storedLocator: ebook.media?.readProgress?.locator,
							percentageCompleted,
						})

				setOpenState({ status: 'ready', opened, initialLocator })
			} catch (error) {
				if (abort.signal.aborted) return
				console.error('[ReadiumWebReader] open failed', error)
				setOpenState({
					status: 'error',
					message:
						error instanceof Error
							? error.message
							: translate('shared.epubReader.errors.openFailed'),
				})
			}
		}

		open()

		return () => abort.abort()
		// eslint-disable-next-line react-compiler/react-compiler
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [id, sdk, isIncognito, t])

	const updateProgress = useCallback(
		(input: EpubProgressInput) => {
			if (isIncognito) return

			const totalSeconds = timer.getCurrentTime()
			const delta = Math.max(0, totalSeconds - lastSyncedElapsedRef.current)

			mutate({
				id: ebook.media?.id || '',
				input: {
					epub: {
						...input,
						elapsedSecondsDelta: delta > 0 ? delta : undefined,
					},
				},
			})
		},
		[mutate, ebook.media?.id, isIncognito, timer],
	)

	const persistLocator = useCallback(
		(locator: Locator, opts?: { isComplete?: boolean }) => {
			const totalProgression = locator.locations.totalProgression
			const percentage =
				opts?.isComplete || (totalProgression != null && totalProgression >= 0.999)
					? 1
					: (totalProgression ?? localProgress ?? 0)

			const chapterTitle = findChapterTitle(locator.href, parseToc(ebook.toc)) ?? locator.title
			const payload = toolkitLocatorToInput(locator, chapterTitle)
			lastSyncedLocatorRef.current = {
				chapterTitle: payload.chapterTitle ?? '',
				href: payload.href,
				title: payload.title,
				type: payload.type ?? 'application/xhtml+xml',
				locations: payload.locations
					? {
							fragments: payload.locations.fragments,
							progression: payload.locations.progression,
							position: payload.locations.position,
							totalProgression: payload.locations.totalProgression,
							cssSelector: payload.locations.cssSelector,
						}
					: null,
				text: payload.text ?? null,
			}

			updateProgress({
				locator: payload,
				percentage,
				isComplete: opts?.isComplete || percentage >= 1,
			})

			setLocalProgress(percentage)
		},
		[ebook.toc, localProgress, updateProgress],
	)

	const debouncedPersist = useDebounce((locator: Locator) => {
		persistLocator(locator)
	}, 500)

	const handlePositionChanged = useCallback(
		(locator: Locator) => {
			setCurrentLocator(locator)

			const total = locator.locations.totalProgression
			if (total != null) {
				setLocalProgress(total)
			}

			const atEnd = total != null && total >= 0.999
			if (atEnd && !hasReachedEndRef.current) {
				hasReachedEndRef.current = true
				persistLocator(locator, { isComplete: true })
				return
			}
			if (!atEnd) {
				hasReachedEndRef.current = false
			}

			debouncedPersist(locator)
		},
		[debouncedPersist, persistLocator],
	)

	// Flush any pending debounced progress write on unmount so recent reading is not lost.
	// `useDebounce` (rooks) returns a lodash `DebouncedFunc`, which synchronously invokes the
	// pending call (if any) rather than waiting out the remaining debounce window.
	useEffect(() => {
		return () => {
			if (!isIncognito) {
				debouncedPersist.flush()
			}
			void Promise.all([
				queryClient.invalidateQueries({ queryKey: ['bookOverview', id], exact: false }),
				queryClient.invalidateQueries({ queryKey: ['keepReading'], exact: false }),
			])
		}
		// eslint-disable-next-line react-compiler/react-compiler
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [id, isIncognito])

	const opened = openState.status === 'ready' ? openState.opened : null
	const initialLocator = openState.status === 'ready' ? openState.initialLocator : undefined

	const toc = useMemo(() => {
		const fromManifest = opened?.publication.manifest.toc?.items
		if (fromManifest?.length) {
			return linksToToc(fromManifest)
		}
		return parseToc(ebook.toc)
	}, [opened, ebook.toc])

	const initialAnnotations = useMemo<EpubAnnotation[]>(
		() => (ebook.annotations ?? []).map(graphqlAnnotationToReaderAnnotation),
		[ebook.annotations],
	)

	const {
		annotations,
		createAnnotation,
		updateAnnotation,
		deleteAnnotation,
		isPending: isAnnotationMutationPending,
	} = useEpubAnnotations({ mediaId: id, isIncognito, initialAnnotations })

	/** What the floating annotation toolbar acts on: new selected text or a tapped annotation */
	const [target, setTarget] = useState<AnnotationTarget | null>(null)
	const [noteDialog, setNoteDialog] = useState<
		| { mode: 'create'; selection: AnnotationSelection }
		| { mode: 'edit'; annotationId: string }
		| null
	>(null)

	const showSelection = useCallback(
		(sel: BasicTextSelection) => {
			if (!sel.text || !containerRef.current) return

			// Capture surrounding text context immediately, before anything else
			// can modify the DOM selection. Readium's web navigator only populates
			// text.highlight — without before/after, matchQuote() always anchors
			// to the first occurrence of repeated text in the chapter.
			const context = extractSelectionContext(containerRef.current, sel.targetFrameSrc)

			const readingOrderHrefs =
				opened?.publication.manifest.readingOrder?.items.map(
					(link) => packagePathFromHref(link.href),
					// ^ links are absolute on web, and relative to package on mobile, so we
					// normalize onto package-relative
				) ?? []
			const chapterTitleFromToc = findChapterTitle(sel.locator?.href ?? '', toc)
			const locator = enrichSelectionLocator({
				selectionLocator: sel.locator ?? null,
				selectedText: sel.text,
				positions: opened?.positions ?? NO_POSITIONS,
				readingOrderHrefs,
				chapterTitleFromToc,
			})
			if (!locator) return

			if (context) {
				locator.text = {
					...locator.text,
					before: context.before ?? locator.text?.before,
					after: context.after ?? locator.text?.after,
				}
			}

			// The whole selection (every line), falling back to the first line the event reports
			const containerRect = containerRef.current.getBoundingClientRect()
			const rect = selectionViewportRect(containerRef.current, sel.targetFrameSrc) ?? {
				x: containerRect.left + sel.x,
				y: containerRect.top + sel.y,
				width: sel.width,
				height: sel.height,
			}
			setTarget({ kind: 'selection', selection: { rect, locator, text: sel.text } })
		},
		[opened, toc],
	)

	// On touch screens the toolbar waits until the selection settles: its handles are still
	// being dragged while it keeps changing
	const isCoarsePointer = useMemo(
		() => typeof window !== 'undefined' && !!window.matchMedia?.('(pointer: coarse)').matches,
		[],
	)
	const settleTimerRef = useRef<number | undefined>(undefined)
	useEffect(() => () => window.clearTimeout(settleTimerRef.current), [])
	const onTextSelected = useCallback(
		(sel: BasicTextSelection) => {
			window.clearTimeout(settleTimerRef.current)
			if (!isCoarsePointer) {
				showSelection(sel)
				return
			}
			settleTimerRef.current = window.setTimeout(() => showSelection(sel), 280)
		},
		[isCoarsePointer, showSelection],
	)

	const onDecorationActivated = useCallback((event: OnDecorationActivatedEvent): boolean => {
		if (event.group !== ANNOTATION_DECORATION_GROUP || !containerRef.current) return false
		setTarget({
			kind: 'annotation',
			annotationId: event.decoration.id,
			rect: decorationEventViewportRect(containerRef.current, event.rect),
		})
		return true
	}, [])

	// The selection was cleared: drop its toolbar (a tapped annotation's stays)
	const onTextCleared = useCallback(() => {
		window.clearTimeout(settleTimerRef.current)
		setTarget((current) => (current?.kind === 'selection' ? null : current))
	}, [])

	const targetRef = useRef(target)
	useEffect(() => {
		targetRef.current = target
	}, [target])

	/** The book's note shown in the popup (a footnote link was followed) */
	const [shownNote, setShownNote] = useState<ShownNote | null>(null)
	const shownNoteRef = useRef(shownNote)
	useEffect(() => {
		shownNoteRef.current = shownNote
	}, [shownNote])

	// A tap on the page (not on a link, an annotation or a selection) closes the toolbar or the
	// note if one is open. Otherwise it is the page's own tap, which the navigation controls
	// turn into a page turn or showing the reader controls on phones
	const onTap = useCallback((tap: ReaderPageTapDetail) => {
		if (targetRef.current) {
			setTarget(null)
			return
		}
		if (shownNoteRef.current) {
			setShownNote(null)
			return
		}
		containerRef.current?.dispatchEvent(
			new CustomEvent<ReaderPageTapDetail>(READER_PAGE_TAP_EVENT, { bubbles: true, detail: tap }),
		)
	}, [])

	// The navigator's API, for the link handling that the navigator itself calls into
	const apiRef = useRef<ReadiumNavigatorApi | null>(null)
	const goToPlace = useCallback(
		(locator: Locator) => apiRef.current?.go(locator) ?? Promise.resolve(false),
		[],
	)
	const getCurrentPlace = useCallback(() => apiRef.current?.currentLocator ?? null, [])
	const {
		canReturn,
		follow: followWithReturn,
		returnBack,
	} = useLinkReturnStack({ goTo: goToPlace, current: getCurrentPlace })
	const [returnHidden, setReturnHidden] = useState(false)

	/** Go to a place a link (or a note's "Go to note") leads to, remembering the way back */
	const followLink = useCallback(
		(to: Locator) => {
			setShownNote(null)
			setReturnHidden(false)
			void followWithReturn(to)
		},
		[followWithReturn],
	)

	/** The book's pages fetched to show a note from another page, by URL */
	const pagesRef = useRef(new Map<string, Promise<Document | null>>())
	const loadPage = useCallback(
		(href: string): Promise<Document | null> => {
			const pages = pagesRef.current
			let page = pages.get(href)
			if (!page) {
				page = createStumpFetch(sdk)(href)
					.then((response) => (response.ok ? response.text() : Promise.reject(response.status)))
					.then(parseBookPage)
					.catch((error) => {
						console.error('[ReadiumWebReader] failed to load a linked page', error)
						pages.delete(href)
						return null
					})
				pages.set(href, page)
			}
			return page
		},
		[sdk],
	)

	/**
	 * A link in the book was followed. Like KOReader: a footnote opens in a popup (the reader
	 * stays put), other links inside the book move the reader (Return or Back come back),
	 * links out of the book open in a new tab.
	 */
	const handleLink = useCallback(
		async (link: ReaderLinkActivation) => {
			if (!opened) return
			const manifest = opened.publication.manifest
			const resourceHrefs = [
				...(manifest.readingOrder?.items ?? []),
				...(manifest.resources?.items ?? []),
			].map((item) => item.href)
			const target = resolvePublicationTarget(link.href, resourceHrefs, opened.positions)
			if (!target) {
				if (isExternalHref(link.href)) {
					window.open(unmangleHref(link.href), '_blank', 'noopener,noreferrer')
				} else {
					toast.error(translate('shared.epubReader.errors.linkUnavailable'))
				}
				return
			}

			const facts = link.facts ?? NO_LINK_FACTS
			const place = targetLocator(target)
			const sameDocument = !!link.document && hrefsMatch(link.document.baseURI, target.resourceHref)
			// Only fetch another page when the link may well be a note
			const mayBeNote = sameDocument || !place || linkLooksLikeNote(facts, target.fragment)
			const page = !mayBeNote
				? null
				: sameDocument
					? (link.document ?? null)
					: await loadPage(target.resourceHref)
			const element = page && target.fragment ? findTargetElement(page, target.fragment) : null

			if (page && isNoteLink(facts, { element, navigable: !!place, fragment: target.fragment })) {
				const context = {
					baseHref: target.resourceHref,
					sourcePath: link.document ? packagePathFromHref(link.document.baseURI) : undefined,
					refId: link.id,
				}
				const note = target.fragment
					? element && extractNote(page, target.fragment, context)
					: extractDocumentBody(page, context)
				if (note) {
					const noteWord = translate('shared.epubReader.links.note')
					setTarget(null)
					setShownNote({
						title: isShortReference(facts.text)
							? `${noteWord} ${facts.text}`
							: facts.text || noteWord,
						html: note.html,
						place,
					})
					return
				}
			}

			if (place) {
				followLink(place)
			} else {
				toast.error(translate('shared.epubReader.errors.linkUnavailable'))
			}
		},
		[opened, loadPage, followLink, translate],
	)

	const onLinkActivated = useCallback(
		(link: ReaderLinkActivation) => {
			// A tap that closes the annotation toolbar does nothing else
			if (targetRef.current) {
				setTarget(null)
				return
			}
			void handleLink(link)
		},
		[handleLink],
	)

	const closeNote = useCallback(() => setShownNote(null), [])

	const { loadState, api } = useReadiumNavigator({
		containerRef,
		publication: opened?.publication ?? null,
		positions: opened?.positions ?? NO_POSITIONS,
		initialLocator,
		allowedDomains: opened?.allowedDomains ?? NO_DOMAINS,
		preferences,
		onPositionChanged: handlePositionChanged,
		onTextSelected,
		onTextCleared,
		onDecorationActivated,
		onTap,
		onLinkActivated,
	})
	useEffect(() => {
		apiRef.current = api
	}, [api])

	// Selecting to the end of the page and resting there moves the view on (KOReader-like):
	// the selection keeps growing into the next page; see `edgeExtend`
	const isFixedLayout = opened?.publication.metadata.effectiveLayout === Layout.fixed
	const announceChapterEnd = useCallback(
		() => toast.info(translate('shared.reader.extend.acrossChapters')),
		[translate],
	)
	const { peek: extendPeek } = useEdgeHoldExtend({
		containerRef,
		enabled: !isIncognito && !isFixedLayout && readingMode !== ReadingMode.ContinuousVertical,
		isTouch: isCoarsePointer,
		onSettled: api.resyncPage,
		onChapterEnd: announceChapterEnd,
	})

	// Each style is drawn in the reader theme's colours, so redraw when the theme changes.
	// Readium mixes up the styles when it updates decorations in place (e.g. an underline
	// turned yellow and the highlights vanished), so a new theme clears them and adds them back
	const decoratedThemeRef = useRef(decorationTheme)
	useEffect(() => {
		if (decoratedThemeRef.current !== decorationTheme) {
			decoratedThemeRef.current = decorationTheme
			api.applyDecorations([], ANNOTATION_DECORATION_GROUP)
		}
		api.applyDecorations(
			annotationsToDecorations(annotations, decorationTheme, opened?.positions ?? NO_POSITIONS),
			ANNOTATION_DECORATION_GROUP,
		)
	}, [api, annotations, opened, decorationTheme])

	// Moving to another place in the book leaves the toolbar's anchor behind, so drop it rather
	// than show a stale rect. Only a real move counts: the navigator also reports the same
	// position again, e.g. after redrawing the annotations
	const currentPlace = currentLocator
		? `${currentLocator.href}|${currentLocator.locations.position ?? ''}|${currentLocator.locations.progression?.toFixed(4) ?? ''}`
		: null
	useEffect(() => {
		setTarget(null)
		setShownNote(null)
	}, [currentPlace])

	const clearSelectionState = useCallback(() => {
		setTarget(null)
		api.clearSelection()
	}, [api])

	const targetAnnotation = useMemo(
		() =>
			target?.kind === 'annotation'
				? (annotations.find((annotation) => annotation.id === target.annotationId) ?? null)
				: null,
		[target, annotations],
	)

	/**
	 * The selection a toolbar button or key acts on, read when it's pressed: Android's handles
	 * may have changed it since the toolbar showed (see `liveFrameSelection`)
	 */
	const liveSelection = useCallback(
		(captured: AnnotationSelection) => liveFrameSelection(containerRef.current, captured),
		[],
	)

	// While the toolbar is up, follow the selection's handles: the toolbar moves with the
	// selection and acts on all of it, not on what the first pointer-up reported
	const hasSelectionTarget = target?.kind === 'selection'
	useEffect(() => {
		if (!hasSelectionTarget) return undefined
		const container = containerRef.current
		if (!container) return undefined
		let timeout: number | undefined
		const sync = () => {
			window.clearTimeout(timeout)
			timeout = window.setTimeout(() => {
				setTarget((current) => {
					if (current?.kind !== 'selection') return current
					const selection = liveSelection(current.selection)
					return selection === current.selection ? current : { kind: 'selection', selection }
				})
			}, 200)
		}
		const documents = [...container.querySelectorAll('iframe')]
			.map((frame) => {
				try {
					return frame.contentDocument
				} catch {
					return null
				}
			})
			.filter((doc): doc is Document => !!doc)
		for (const doc of documents) doc.addEventListener('selectionchange', sync)
		return () => {
			window.clearTimeout(timeout)
			for (const doc of documents) doc.removeEventListener('selectionchange', sync)
		}
	}, [hasSelectionTarget, liveSelection])

	/** Draw the selection in a style, or restyle the tapped annotation */
	const applyStyle = useCallback(
		(style: AnnotationStyle) => {
			if (target?.kind === 'selection') {
				void createAnnotation(liveSelection(target.selection).locator, { style })
				clearSelectionState()
			} else if (targetAnnotation) {
				if (targetAnnotation.style !== style && !isOptimisticAnnotationId(targetAnnotation.id)) {
					void updateAnnotation(targetAnnotation.id, { style })
				}
				setTarget(null)
			}
		},
		[
			target,
			targetAnnotation,
			createAnnotation,
			updateAnnotation,
			clearSelectionState,
			liveSelection,
		],
	)

	const openNote = useCallback(() => {
		if (target?.kind === 'selection') {
			setNoteDialog({ mode: 'create', selection: liveSelection(target.selection) })
		} else if (targetAnnotation && !isOptimisticAnnotationId(targetAnnotation.id)) {
			setNoteDialog({ mode: 'edit', annotationId: targetAnnotation.id })
		}
	}, [target, targetAnnotation, liveSelection])

	const copyTarget = useCallback(async () => {
		const text =
			target?.kind === 'selection'
				? liveSelection(target.selection).text
				: (targetAnnotation?.locator.text?.highlight ?? '')
		if (!text) return
		if (await copyText(text)) {
			toast.success(translate('shared.readerAnnotations.copied'))
			clearSelectionState()
		} else {
			toast.error(translate('shared.readerAnnotations.copyFailed'))
		}
	}, [target, targetAnnotation, translate, clearSelectionState, liveSelection])

	const deleteTargetAnnotation = useCallback(() => {
		if (targetAnnotation && !isOptimisticAnnotationId(targetAnnotation.id)) {
			void deleteAnnotation(targetAnnotation.id)
		}
		setTarget(null)
	}, [targetAnnotation, deleteAnnotation])

	const handleAnnotationShortcut = useCallback(
		(action: AnnotationShortcutAction) => {
			switch (action.type) {
				case 'copy':
					void copyTarget()
					break
				case 'style':
					applyStyle(action.style)
					break
				case 'note':
					openNote()
					break
				case 'delete':
					deleteTargetAnnotation()
					break
				case 'dismiss':
					if (target?.kind === 'selection') {
						clearSelectionState()
					} else {
						setTarget(null)
					}
					break
			}
		},
		[applyStyle, openNote, deleteTargetAnnotation, target, clearSelectionState, copyTarget],
	)

	const noteAnnotation = useMemo(
		() =>
			noteDialog?.mode === 'edit'
				? (annotations.find((annotation) => annotation.id === noteDialog.annotationId) ?? null)
				: null,
		[noteDialog, annotations],
	)

	const handleCloseNoteDialog = useCallback(() => {
		setNoteDialog(null)
	}, [])

	const handleSaveNote = useCallback(
		(note: string, style: AnnotationStyle) => {
			if (noteDialog?.mode === 'create') {
				void createAnnotation(noteDialog.selection.locator, {
					annotationText: note || undefined,
					style,
				})
				clearSelectionState()
			} else if (noteDialog?.mode === 'edit' && noteAnnotation) {
				void updateAnnotation(noteAnnotation.id, { annotationText: note || null, style })
				setTarget(null)
			}
			setNoteDialog(null)
		},
		[noteDialog, noteAnnotation, createAnnotation, updateAnnotation, clearSelectionState],
	)

	const handleDeleteFromNoteDialog = useCallback(() => {
		if (noteAnnotation) {
			void deleteAnnotation(noteAnnotation.id)
		}
		setNoteDialog(null)
		setTarget(null)
	}, [noteAnnotation, deleteAnnotation])

	const existingBookmarks = useMemo(() => {
		const map: Record<string, Bookmark> = {}
		for (const bookmark of ebook.bookmarks ?? []) {
			const key = bookmark.locator?.href
				? `${packageKey(bookmark.locator.href)}:${bookmark.locator.locations?.position ?? ''}`
				: bookmark.id
			map[key] = bookmark
		}
		return map
	}, [ebook.bookmarks])

	// The reading bar: where the chapters start (its ticks) and how many pages of the chapter
	// are left, measured in the book's frame (pages that turn: screens across the chapter;
	// scrolling: screen heights down it)
	const tocStarts = useMemo(() => {
		const positions = opened?.positions ?? NO_POSITIONS
		if (!positions.length) return []
		return flattenTocWithPaths(toc).flatMap(({ item, path }) => {
			const base = item.content.split('#')[0] ?? item.content
			const first = positions.find((position) => hrefsMatch(position.href, base))
			const start = first?.locations.totalProgression
			return typeof start === 'number' ? [{ start, path, depth: path.length }] : []
		})
	}, [opened, toc])
	const chapterStarts = useMemo(
		() =>
			[...new Set(tocStarts.filter((entry) => entry.depth === 1).map((entry) => entry.start))].sort(
				(a, b) => a - b,
			),
		[tocStarts],
	)

	const [chapterPageCount, setChapterPageCount] = useState<ChapterPages | null>(null)
	const isScrolled = readingMode === ReadingMode.ContinuousVertical
	useEffect(() => {
		const container = containerRef.current
		if (!container || !currentLocator) return undefined
		const measure = () => {
			const measured = measureChapterPages(container, isScrolled)
			if (measured) {
				setChapterPageCount(measured)
				return
			}
			// the frame can't be read: count Readium's positions (about a page each) instead
			const positions = opened?.positions ?? NO_POSITIONS
			const inChapter = positions.filter((position) =>
				hrefsMatch(position.href, currentLocator.href),
			)
			const here = currentLocator.locations.position ?? 0
			const index = Math.max(
				0,
				inChapter.findIndex((position) => position.locations.position === here),
			)
			setChapterPageCount(
				inChapter.length
					? { page: index + 1, pages: inChapter.length, left: inChapter.length - index - 1 }
					: null,
			)
		}
		// after Readium has settled the page it reported
		const frame = window.requestAnimationFrame(() => window.requestAnimationFrame(measure))
		window.addEventListener('resize', measure)
		return () => {
			window.cancelAnimationFrame(frame)
			window.removeEventListener('resize', measure)
		}
	}, [currentLocator, isScrolled, opened])

	const chapterMeta = useMemo(() => {
		const chapterTitle = findChapterTitle(currentLocator?.href ?? '', toc) ?? currentLocator?.title
		const readingOrder = opened?.publication.manifest.readingOrder?.items ?? []
		const sectionSpineIndex = currentLocator
			? readingOrder.findIndex((l) => hrefsMatch(l.href, currentLocator.href))
			: -1
		const locatorPosition = currentLocator?.locations.position
		const totalPositions = opened?.positions.length

		const readerLocator = currentLocator
			? toolkitLocatorToReaderLocator(currentLocator, chapterTitle ?? undefined)
			: null

		const href = currentLocator?.href ?? ''
		const path = href
			? tocPath(
					toc,
					(item) =>
						hrefsMatch(item.content, href) || item.content.endsWith(href.split('#')[0] ?? ''),
				)
			: []

		// the contents' links don't always name the page's file: the entry whose start is the
		// last one at or before the position (the deepest when several start there) instead
		const at = currentLocator?.locations.totalProgression
		const byPosition =
			at == null
				? undefined
				: tocStarts.reduce<(typeof tocStarts)[number] | undefined>(
						(best, entry) =>
							entry.start <= at + 1e-9 &&
							(!best ||
								entry.start > best.start ||
								(entry.start === best.start && entry.depth >= best.depth))
								? entry
								: best,
						undefined,
					)

		return {
			name: chapterTitle,
			path: path.length
				? path
				: byPosition?.path.length
					? byPosition.path
					: chapterTitle
						? [chapterTitle]
						: [],
			pageInChapter: chapterPageCount?.page,
			pagesInChapter: chapterPageCount?.pages,
			pagesLeftInChapter: chapterPageCount?.left,
			position: sectionSpineIndex >= 0 ? sectionSpineIndex : undefined,
			sectionSpineIndex: sectionSpineIndex >= 0 ? sectionSpineIndex : undefined,
			totalProgression: currentLocator?.locations.totalProgression,
			locatorPosition: locatorPosition ?? undefined,
			totalPositions,
			currentLocator: readerLocator,
		}
	}, [currentLocator, toc, opened, chapterPageCount, tocStarts])

	const onPaginateForward = useCallback(() => {
		void api.goForward()
	}, [api])

	const onPaginateBackward = useCallback(() => {
		void api.goBackward()
	}, [api])

	const onGoToLocator = useCallback(
		async (locator: ReaderLocator): Promise<boolean> => {
			if (!opened) return false
			try {
				const toolkit = resolveInitialLocator({
					positions: opened.positions,
					storedLocator: {
						chapterTitle: locator.chapterTitle ?? '',
						href: locator.href,
						title: locator.title,
						type: locator.type || 'application/xhtml+xml',
						locations: locator.locations
							? {
									fragments: locator.locations.fragments,
									progression: locator.locations.progression,
									position: locator.locations.position,
									totalProgression: locator.locations.totalProgression,
									cssSelector: locator.locations.cssSelector ?? null,
								}
							: null,
						text: locator.text ?? null,
					},
				})
				if (toolkit) {
					const navigated = await api.go(toolkit)
					if (navigated) return true
				}
			} catch (err) {
				console.error(err)
			}

			toast.error(translate('shared.epubReader.errors.navigateFailed'))
			return false
		},
		[api, opened, translate],
	)

	const onLinkClick = useCallback(
		(href: string) => {
			if (!opened || !api) return
			const readingOrder = opened.publication.manifest.readingOrder?.items ?? []
			const resources = opened.publication.manifest.resources?.items ?? []
			const tocItems = flattenTocLinks(opened.publication.manifest.toc?.items ?? [])
			const match =
				tocItems.find((l) => hrefsMatch(l.href, href) || l.href.endsWith(href)) ??
				readingOrder.find((l) => hrefsMatch(l.href, href) || l.href.endsWith(href)) ??
				resources.find((l) => hrefsMatch(l.href, href) || l.href.endsWith(href))

			if (match) {
				void api.goLink(match)
				return
			}

			// Fall back to first position with matching package path
			const position = opened.positions.find((p) => hrefsMatch(p.href, href))
			if (position) {
				void api.go(position)
				return
			}

			toast.error('Could not find that location in the publication')
		},
		[api, opened],
	)

	const jumpToSection = useCallback(
		(section: number) => {
			const item = opened?.publication.manifest.readingOrder?.items?.[section]
			if (item) {
				void api.goLink(item)
			}
		},
		[api, opened],
	)

	const getLocatorPreviewText = useCallback((locator: ReaderLocator) => {
		return locator.text?.highlight ?? locator.chapterTitle ?? locator.title ?? null
	}, [])

	/**
	 * Server-backed whole-book search — never downloads the EPUB archive, just queries
	 * the manifest-scoped search endpoint for Readium locators.
	 */
	const searchBook = useCallback(
		(
			query: string,
			opts?: { cursor?: string; signal?: AbortSignal },
		): Promise<EpubSearchResponse> =>
			sdk.epub.search({ id, q: query, cursor: opts?.cursor, signal: opts?.signal }),
		[sdk, id],
	)

	/** Keyboard navigation — RTL aware, ignores keystrokes aimed at inputs/dialogs */
	useEffect(() => {
		const isLtr = readingDirection !== ReadingDirection.Rtl

		const handleKeyDown = (event: KeyboardEvent) => {
			if (isEditableTarget(event.target)) return
			// Shift+arrows extend a text selection, other modifiers belong to the browser
			if (event.shiftKey || event.ctrlKey || event.metaKey || event.altKey) return

			const nextKey = isLtr ? 'ArrowRight' : 'ArrowLeft'
			const prevKey = isLtr ? 'ArrowLeft' : 'ArrowRight'
			if (event.key === nextKey) {
				event.preventDefault()
				void api.goForward()
			} else if (event.key === prevKey) {
				event.preventDefault()
				void api.goBackward()
			}
		}

		window.addEventListener('keydown', handleKeyDown, { capture: true })
		return () => window.removeEventListener('keydown', handleKeyDown, { capture: true })
	}, [api, readingDirection])

	if (!ebook.media) {
		return null
	}

	const isLoading =
		openState.status === 'loading' ||
		(openState.status === 'ready' && loadState.status === 'loading')
	const errorMessage =
		openState.status === 'error'
			? openState.message
			: loadState.status === 'error'
				? loadState.message
				: null

	return (
		<EpubReaderContainer
			isFixedLayout={isFixedLayout}
			readerMeta={{
				bookEntity: ebook.media,
				bookMeta: {
					bookmarks: existingBookmarks,
					annotations,
					chapter: chapterMeta,
					chapterStarts,
					toc,
				},
				progress: localProgress,
			}}
			controls={{
				getLocatorPreviewText,
				onGoToLocator,
				onLinkClick,
				onPaginateBackward,
				onPaginateForward,
				jumpToSection,
				searchBook,
				canGoForward: api.canGoForward,
				canGoBackward: api.canGoBackward,
			}}
		>
			{/*
			  This outer div is the "viewport parent" — measured independently via
			  ResizeObserver above to decide the effective column count, and unaffected by
			  the centering max-width applied to the inner div below. md:px-12 matches the
			  chevron gutters (w-12) rendered by EpubNavigationControls, one level up.
			*/}
			<div
				ref={viewportRef}
				className="min-h-0 md:px-12 relative h-full w-full flex-1 self-stretch overflow-hidden"
			>
				{/*
			  Readium observes *this* element (container.parentElement) and sets the
			  inner host's width. Keep it CSS-sized and stable — AutoSizer 0×0 flashes +
			  React size state fight that ResizeObserver path. Readium handles column
			  width and pagination internally via ReadiumCSS.paginate(), so no
			  max-width constraint is needed here. The epub background color is set on
			  this parent so it fills full width even in paged mode, where Readium
			  narrows the container to the column reading width.
			*/}
				<div
					className="relative mx-auto h-full w-full overflow-hidden"
					style={{
						backgroundColor: themeColors.background,
						// the text stays clear of a camera cutout or rounded corners (portrait and
						// landscape, paged and scrolled); Readium lays the pages out inside the
						// padding, which keeps the page colour
						paddingTop: 'env(safe-area-inset-top)',
						// the reading bar, when it stays, has its own room: it never covers text
						paddingBottom: reserveReadingBar ? READING_BAR_ZONE : 'env(safe-area-inset-bottom)',
						paddingLeft: 'env(safe-area-inset-left)',
						paddingRight: 'env(safe-area-inset-right)',
					}}
				>
					<div
						ref={containerRef}
						className="relative mx-auto h-full w-full overflow-hidden"
						data-epub-column
					/>
				</div>

				{isLoading && (
					<div className="inset-0 absolute z-10 flex items-center justify-center bg-background/80">
						<Spinner />
					</div>
				)}

				{errorMessage && (
					<div className="inset-0 gap-2 p-6 absolute z-10 flex flex-col items-center justify-center bg-background text-center">
						<p className="text-sm text-foreground">{errorMessage}</p>
					</div>
				)}
			</div>

			<EpubReaderShortcuts
				hasTarget={!!target && !isIncognito}
				targetIsAnnotation={target?.kind === 'annotation'}
				onAnnotationAction={handleAnnotationShortcut}
			/>

			{target &&
				!isIncognito &&
				!noteDialog &&
				(target.kind === 'selection' || targetAnnotation) && (
					<AnnotationToolbar
						// A new target starts collapsed
						key={target.kind === 'selection' ? target.selection.text : target.annotationId}
						anchor={target.kind === 'selection' ? target.selection.rect : target.rect}
						mode={target.kind}
						activeStyle={targetAnnotation?.style}
						hasNote={!!targetAnnotation?.annotationText}
						onStyle={applyStyle}
						onNote={openNote}
						onCopy={() => void copyTarget()}
						onDelete={target.kind === 'annotation' ? deleteTargetAnnotation : undefined}
					/>
				)}

			{extendPeek && (
				<div
					role="status"
					data-testid="extend-peek"
					className="gap-0.5 px-3 py-2 text-sm shadow-lg pointer-events-none fixed left-1/2 z-50 flex w-max max-w-[min(34rem,calc(100%-1rem))] -translate-x-1/2 flex-col rounded-lg border border-border bg-background/95 text-foreground"
					style={{ top: 'calc(env(safe-area-inset-top) + 0.5rem)' }}
				>
					<span className="text-[11px] text-muted-foreground">
						{translate('shared.reader.extend.continues')}
					</span>
					<span className="line-clamp-2">
						<mark className="rounded-sm bg-primary/20 text-foreground">{extendPeek}</mark>
					</span>
				</div>
			)}

			{shownNote && (
				<NotePopup
					note={shownNote}
					onClose={closeNote}
					onGoToNote={
						shownNote.place ? () => shownNote.place && followLink(shownNote.place) : undefined
					}
				/>
			)}

			{canReturn && !returnHidden && !shownNote && (
				<ReturnChip onReturn={returnBack} onHide={() => setReturnHidden(true)} />
			)}

			<AnnotationNoteDialog
				open={noteDialog !== null}
				mode={noteDialog?.mode ?? 'create'}
				quotedText={
					noteDialog?.mode === 'edit'
						? noteAnnotation?.locator.text?.highlight
						: noteDialog?.selection.text
				}
				initialNote={noteDialog?.mode === 'edit' ? noteAnnotation?.annotationText : undefined}
				initialStyle={noteAnnotation?.style ?? AnnotationStyle.Highlight}
				isPending={isAnnotationMutationPending}
				onOpenChange={(open) => !open && handleCloseNoteDialog()}
				onSave={handleSaveNote}
				onDelete={noteDialog?.mode === 'edit' ? handleDeleteFromNoteDialog : undefined}
			/>
		</EpubReaderContainer>
	)
}

function packageKey(href: string): string {
	try {
		const url = new URL(href, 'https://stump.invalid')
		const marker = '/resource/'
		const idx = url.pathname.indexOf(marker)
		if (idx >= 0) return decodeURIComponent(url.pathname.slice(idx + marker.length))
		return decodeURIComponent(url.pathname.replace(/^\//, ''))
	} catch {
		return href.split('#')[0] ?? href
	}
}

function parseToc(toc: string[] | null | undefined): EpubContent[] {
	if (!toc) return []
	return toc
		.map((item) => {
			try {
				return JSON.parse(item) as EpubContent
			} catch {
				return null
			}
		})
		.filter((item): item is EpubContent => item !== null)
}

function linksToToc(links: Link[], playOrder = { value: 0 }): EpubContent[] {
	return links.map((link) => {
		const order = playOrder.value++
		return {
			label: link.title || link.href,
			content: link.href,
			play_order: order,
			children: link.children?.items ? linksToToc(link.children.items, playOrder) : [],
		}
	})
}

function flattenTocLinks(links: Link[]): Link[] {
	const out: Link[] = []
	for (const link of links) {
		out.push(link)
		if (link.children?.items?.length) {
			out.push(...flattenTocLinks(link.children.items))
		}
	}
	return out
}

function findChapterTitle(href: string, toc: EpubContent[]): string | undefined {
	if (!href) return undefined
	const stack = [...toc]
	while (stack.length) {
		const item = stack.shift()!
		if (hrefsMatch(item.content, href) || item.content.endsWith(href.split('#')[0] ?? '')) {
			return item.label.trim()
		}
		stack.push(...item.children)
	}
	return undefined
}

/**
 * The pages of the chapter on screen, read from the book's frame that shows: pages that
 * turn are screens across it, scrolling is screen heights down it (`null` when there's no
 * frame to read)
 */
function measureChapterPages(container: HTMLElement, scrolled: boolean): ChapterPages | null {
	const frames = [...container.querySelectorAll('iframe')].filter(
		(frame) => frame.style.visibility !== 'hidden',
	)
	for (const frame of frames.reverse()) {
		try {
			const view = frame.contentWindow
			const scroller = view?.document.scrollingElement as HTMLElement | null | undefined
			if (!view || !scroller) continue
			const measured = scrolled
				? chapterPages({
						scrollSize: scroller.scrollHeight,
						viewportSize: view.innerHeight,
						offset: scroller.scrollTop,
					})
				: chapterPages({
						scrollSize: scroller.scrollWidth,
						viewportSize: view.innerWidth,
						offset: scroller.scrollLeft,
					})
			if (measured) return measured
		} catch {
			// a frame from another origin, or one being torn down
		}
	}
	return null
}

/** Every contents entry in reading order, with the labels from the top down to it */
function flattenTocWithPaths(
	toc: EpubContent[],
	parents: string[] = [],
): { item: EpubContent; path: string[] }[] {
	return toc.flatMap((item) => {
		const path = [...parents, item.label.trim()]
		return [{ item, path }, ...flattenTocWithPaths(item.children, path)]
	})
}
