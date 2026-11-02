import { type ReaderShortcutAction, resolveReaderShortcut } from '../annotations'
import { type PanDirection, panDirectionOf } from '../shared/panKeys'

/** What a key press asks the PDF reader to do (besides the shared annotation shortcuts) */
export type PdfKeyAction =
	| { type: 'previous' }
	| { type: 'next' }
	| { type: 'screenUp' }
	| { type: 'screenDown' }
	| { type: 'first' }
	| { type: 'last' }
	| { type: 'zoomIn' }
	| { type: 'zoomOut' }
	| { type: 'zoomReset' }
	| { type: 'toggleFit' }
	| { type: 'goToPage' }
	/** A zoomed page moves (or turns at its edge); `screen`: by most of a screen (Space) */
	| { type: 'pan'; direction: PanDirection; screen?: boolean }

export type PdfShortcut = ReaderShortcutAction | PdfKeyAction

type KeyLike = Pick<KeyboardEvent, 'key' | 'ctrlKey' | 'metaKey' | 'altKey' | 'shiftKey'>

type Context = {
	/** Text or an annotation is selected (the annotation toolbar is open) */
	hasTarget: boolean
	targetIsAnnotation: boolean
	/** Continuous scroll: Space and Shift+Space scroll a screen instead of turning a page */
	continuous: boolean
	/**
	 * A zoomed page in page-by-page mode: the arrows (and Space) move around it, turning the
	 * page at its edge; Page Up / Page Down still turn pages
	 */
	zoomedPage?: boolean
}

/**
 * Maps a key press to a PDF reader action. The shared reader shortcuts (`h u s i n c Del b
 * d ? Esc`, Ctrl/Cmd+F or `/` to search; see `resolveReaderShortcut`) come first; then the
 * page keys:
 *
 * - `←` / `PageUp` previous page, `→` / `PageDown` next page
 * - `Space` / `Shift+Space` next / previous screen (continuous) or page (paged)
 * - a zoomed page (paged): `← → ↑ ↓` and `Space` move around it and turn the page at its
 *   edge; `PageUp` / `PageDown` still turn pages
 * - `Home` / `End` first / last page
 * - `+` / `-` / `0` zoom in, out, reset; `f` fit width ↔ fit page; `g` go to page
 *
 * The letters don't clash with the shared ones (`h u s i n c b d`).
 */
export function resolvePdfShortcut(event: KeyLike, context: Context): PdfShortcut | null {
	const shared = resolveReaderShortcut(event, context)
	if (shared) return shared

	if (event.ctrlKey || event.metaKey || event.altKey) return null

	if (context.zoomedPage && !context.continuous) {
		const direction = panDirectionOf(event.key)
		if (direction) return { type: 'pan', direction }
		if (event.key === ' ' || event.key === 'Spacebar') {
			return { type: 'pan', direction: { dx: 0, dy: event.shiftKey ? -1 : 1 }, screen: true }
		}
	}

	switch (event.key) {
		case 'ArrowLeft':
		case 'PageUp':
			return { type: 'previous' }
		case 'ArrowRight':
		case 'PageDown':
			return { type: 'next' }
		case ' ':
		case 'Spacebar':
			if (context.continuous) return { type: event.shiftKey ? 'screenUp' : 'screenDown' }
			return { type: event.shiftKey ? 'previous' : 'next' }
		case 'Home':
			return { type: 'first' }
		case 'End':
			return { type: 'last' }
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
