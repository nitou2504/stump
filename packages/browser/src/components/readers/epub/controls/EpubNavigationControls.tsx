import { cx } from '@stump/components'
import { ReadingDirection, ReadingMode } from '@stump/graphql'
import { ChevronLeft, ChevronRight } from 'lucide-react'
import { useCallback, useEffect, useRef } from 'react'

import { useTranslate } from '@/hooks/useTranslate'
import { useBookPreferences } from '@/scenes/book/reader/useBookPreferences'

import { navigationTapZone } from '../../shared/gestures'
import { useSwipeTurn } from '../../shared/useSwipeTurn'
import { useEpubReaderContext, useEpubReaderControls } from '../context'
import ControlButton from './ControlButton'
import { READER_PAGE_TAP_EVENT, type ReaderPageTapDetail } from './pageTap'

/** Phones and small tablets: below Tailwind's `md` breakpoint, where the chevrons are hidden */
const PHONE_LAYOUT_QUERY = '(max-width: 767.98px)'
/** Any touch screen, e.g. a phone turned sideways (wider than `md`), which has no mouse to hover */
const TOUCH_QUERY = '(pointer: coarse)'

type Props = {
	children: React.ReactNode
}

/**
 * Navigation chrome for the EPUB reader stage: desktop hover chevrons, and on phones taps on
 * the page itself: the side strips turn the page and the middle shows or hides the reader
 * controls. Those taps come from inside the book's frame ({@link READER_PAGE_TAP_EVENT}),
 * which only reports taps that aren't on a link, an annotation or selected text, so nothing
 * covers the text: it can be selected (long press) and its links work. Everything here is
 * positioned `absolute` relative to the stage (this component's own root), not `fixed` to the
 * viewport — so it tracks the actual reading pane instead of the window.
 */
export default function EpubNavigationControls({ children }: Props) {
	const { translate } = useTranslate()
	const {
		readerMeta: { bookEntity: book },
	} = useEpubReaderContext()
	const { toggleVisible, onPaginateBackward, onPaginateForward, canGoBackward, canGoForward } =
		useEpubReaderControls()
	const {
		bookPreferences: { readingDirection, readingMode, tapSidesToNavigate },
	} = useBookPreferences({ book })

	const invertControls = readingDirection === ReadingDirection.Rtl
	const isVerticalScrolling = readingMode === ReadingMode.ContinuousVertical

	/**
	 * A callback to navigate backward in the book, wrt the natural reading
	 * progression direction.
	 *
	 * If the reading direction is RTL, then the backward navigation is actually
	 * forward in the book.
	 */
	const onBackwardNavigation = useCallback(() => {
		if (invertControls) {
			onPaginateForward()
		} else {
			onPaginateBackward()
		}
	}, [invertControls, onPaginateBackward, onPaginateForward])

	/**
	 * A callback to navigate forward in the book, wrt the natural reading
	 * progression direction.
	 *
	 * If the reading direction is RTL, then the forward navigation is actually
	 * backwards in the book.
	 */
	const onForwardNavigation = useCallback(() => {
		if (invertControls) {
			onPaginateBackward()
		} else {
			onPaginateForward()
		}
	}, [invertControls, onPaginateBackward, onPaginateForward])

	// canGoForward/canGoBackward are book-progression relative; map them to the physical
	// (left/right) chevrons, which are inverted under RTL.
	const canNavigateLeft = invertControls ? canGoForward !== false : canGoBackward !== false
	const canNavigateRight = invertControls ? canGoBackward !== false : canGoForward !== false

	const toggleControls = toggleVisible

	const rootRef = useRef<HTMLDivElement | null>(null)
	const pageTapRef = useRef<(tap: ReaderPageTapDetail) => void>(() => {})
	useEffect(() => {
		pageTapRef.current = ({ xFraction }) => {
			// Desktop keeps its chevrons and arrow keys; a click on the page does nothing. A touch
			// screen taps at any width: a phone in landscape is wider than the phone layout
			const touchLayout =
				!!window.matchMedia?.(PHONE_LAYOUT_QUERY).matches ||
				!!window.matchMedia?.(TOUCH_QUERY).matches
			if (!touchLayout) return

			// In the scroll layout the sides have no page to turn ('forward' there is the next
			// chapter): every tap shows or hides the controls, scrolling moves through the book
			// The tap zones every reader shares: a third of the width at each side of a touch
			// screen. (With a mouse the page isn't a tap target: a click there starts selecting
			// text, and the chevrons in the margins turn the page.)
			const zone = navigationTapZone(xFraction, {
				flow: isVerticalScrolling ? 'continuous' : 'paged',
				coarse: true,
				sidesNavigate: !!tapSidesToNavigate,
			})
			if (zone === 'left' && canNavigateLeft) {
				onBackwardNavigation()
			} else if (zone === 'right' && canNavigateRight) {
				onForwardNavigation()
			} else if (zone === 'middle') {
				toggleControls()
			}
		}
	})
	useEffect(() => {
		const root = rootRef.current
		if (!root) return
		const onPageTap = (event: Event) =>
			pageTapRef.current((event as CustomEvent<ReaderPageTapDetail>).detail)
		root.addEventListener(READER_PAGE_TAP_EVENT, onPageTap)
		return () => root.removeEventListener(READER_PAGE_TAP_EVENT, onPageTap)
	}, [])

	/**
	 * A deliberate sideways swipe outside the book's frame (its margins) turns the page with
	 * the rule every reader shares (`useSwipeTurn`); in the scroll layout scrolling is the way
	 * through the book. Swipes on the text itself are Readium's: it slides its columns with the
	 * finger and keeps a long-press text selection working.
	 */
	useSwipeTurn(rootRef, {
		enabled: !isVerticalScrolling,
		// the screen's right is 'forward' here (mirrored for right-to-left books above)
		onSwipe: (screenTurn) => (screenTurn === 1 ? onForwardNavigation() : onBackwardNavigation()),
	})

	return (
		<div ref={rootRef} className="min-h-0 relative h-full w-full flex-1">
			{/* Desktop chevrons in the side gutters (the one that can't turn is dimmed) */}
			<div
				className={cx('group inset-y-0 left-0 w-12 absolute z-20 hidden items-center', {
					'md:flex': true,
				})}
			>
				<ControlButton
					onClick={onBackwardNavigation}
					disabled={!canNavigateLeft}
					aria-label={translate('shared.epubReader.controls.previousPage')}
				>
					<ChevronLeft className="h-5 w-5" />
				</ControlButton>
			</div>
			<div
				className={cx(
					'group inset-y-0 right-0 w-12 absolute z-20 hidden items-center justify-end',
					{
						'md:flex': true,
					},
				)}
			>
				<ControlButton
					onClick={onForwardNavigation}
					disabled={!canNavigateRight}
					aria-label={translate('shared.epubReader.controls.nextPage')}
				>
					<ChevronRight className="h-5 w-5" />
				</ControlButton>
			</div>

			{/*
		  Phones turn the page and show the controls by tapping the page itself (see above), so
		  no tap zone covers the text. These stay for screen readers and keyboards.
		*/}
			<div className="md:hidden">
				<button
					type="button"
					className="sr-only"
					aria-label={translate('shared.epubReader.controls.previousPage')}
					disabled={!canNavigateLeft}
					onClick={onBackwardNavigation}
				/>
				<button
					type="button"
					className="sr-only"
					aria-label={translate('shared.epubReader.controls.toggleControls')}
					onClick={toggleControls}
				/>
				<button
					type="button"
					className="sr-only"
					aria-label={translate('shared.epubReader.controls.nextPage')}
					disabled={!canNavigateRight}
					onClick={onForwardNavigation}
				/>
			</div>

			{children}
		</div>
	)
}
