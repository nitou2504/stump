import { type ReaderShortcutAction, resolveReaderShortcut } from '../annotations'
import { panDirectionOf } from '../shared/panKeys'
import { type ScrollKeyAction, scrollKeyOf } from '../shared/scrollKeys'

/** What a key press asks the PDF reader to do (besides the shared annotation shortcuts) */
export type PdfKeyAction =
	| { type: 'previous' }
	| { type: 'next' }
	| { type: 'first' }
	| { type: 'last' }
	| { type: 'zoomIn' }
	| { type: 'zoomOut' }
	| { type: 'zoomReset' }
	| { type: 'toggleFit' }
	| { type: 'goToPage' }
	/** Moves the pages (a line, a screen), stopping at their edges: never turns a page */
	| { type: 'scroll'; action: ScrollKeyAction }
	/**
	 * Space in the page-by-page layout: the next (or previous) screen of a page taller than
	 * the screen, then the next (or previous) page
	 */
	| { type: 'step'; delta: 1 | -1 }

export type PdfShortcut = ReaderShortcutAction | PdfKeyAction

type KeyLike = Pick<KeyboardEvent, 'key' | 'ctrlKey' | 'metaKey' | 'altKey' | 'shiftKey'>

type Context = {
	/** Text or an annotation is selected (the annotation toolbar is open) */
	hasTarget: boolean
	targetIsAnnotation: boolean
	/** The pages scroll (one long strip) rather than turn one at a time */
	continuous: boolean
	/** Zoomed in past the fit, in the page-by-page layout */
	zoomedPage?: boolean
}

/**
 * Maps a key press to a PDF reader action. The shared reader shortcuts (`h u s i n c Del b
 * d ? Esc`, Ctrl/Cmd+F or `/` to search; see `resolveReaderShortcut`) come first; then:
 *
 * - **Pages that turn**: `←` / `PageUp` previous page, `→` / `PageDown` next page; `↑` / `↓`
 *   move down a page taller than the screen; `Space` / `Shift+Space` the next / previous
 *   screen of such a page, then the page
 * - **A zoomed page** (pages that turn): the arrows and `Space` move around it and stop at
 *   its edges, nothing else; only `PageUp` / `PageDown` turn pages
 * - **Pages that scroll**: `↑` / `↓` a line, `Space` / `Shift+Space` and `PageDown` /
 *   `PageUp` a screen, `←` / `→` sideways when zoomed wider than the screen
 * - `Home` / `End` the first / last page; `+` / `-` / `0` zoom in, out, reset; `f` fit width
 *   ↔ fit page; `g` go to page
 *
 * The letters don't clash with the shared ones (`h u s i n c b d`).
 */
export function resolvePdfShortcut(event: KeyLike, context: Context): PdfShortcut | null {
	const shared = resolveReaderShortcut(event, context)
	if (shared) return shared

	if (event.ctrlKey || event.metaKey || event.altKey) return null

	const scroll = scrollKeyOf(event)
	if (scroll?.type === 'edge') return { type: scroll.to === 'start' ? 'first' : 'last' }

	if (context.continuous) {
		if (scroll) return { type: 'scroll', action: scroll }
	} else if (context.zoomedPage) {
		if (event.key === 'PageUp') return { type: 'previous' }
		if (event.key === 'PageDown') return { type: 'next' }
		if (scroll) return { type: 'scroll', action: scroll }
	} else {
		switch (event.key) {
			case 'ArrowLeft':
			case 'PageUp':
				return { type: 'previous' }
			case 'ArrowRight':
			case 'PageDown':
				return { type: 'next' }
			case ' ':
			case 'Spacebar':
				return { type: 'step', delta: event.shiftKey ? -1 : 1 }
			default:
				break
		}
		const direction = event.shiftKey ? null : panDirectionOf(event.key)
		if (direction && direction.dy !== 0) {
			return { type: 'scroll', action: { type: 'line', dx: 0, dy: direction.dy } }
		}
	}

	switch (event.key) {
		case '+':
		case '=':
			return { type: 'zoomIn' }
		case '-':
		case '_':
			return { type: 'zoomOut' }
		case '0':
			return { type: 'zoomReset' }
		default:
			break
	}

	const key = event.key.length === 1 ? event.key.toLowerCase() : event.key
	if (key === 'f') return { type: 'toggleFit' }
	if (key === 'g') return { type: 'goToPage' }
	return null
}
