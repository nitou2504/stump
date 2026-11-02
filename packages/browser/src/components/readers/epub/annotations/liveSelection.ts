import type { AnnotationSelection } from '../../annotations/types'
import { extractSelectionContext, selectionViewportRect } from './locator'

/**
 * The non-empty selection inside one of the reader's frames, if any; with `text`, the frame
 * whose selection is that text is preferred (another chapter's frame may still hold one)
 */
export function findFrameSelection(
	container: HTMLElement | null,
	text?: string,
): { document: Document; range: Range } | null {
	if (!container) return null
	let first: { document: Document; range: Range } | null = null
	for (const frame of container.querySelectorAll('iframe')) {
		try {
			const document = frame.contentDocument
			const selection = document?.getSelection()
			if (document && selection && selection.rangeCount > 0 && !selection.isCollapsed) {
				const found = { document, range: selection.getRangeAt(0).cloneRange() }
				if (!text || selection.toString().trim() === text.trim()) return found
				first ??= found
			}
		} catch {
			// a cross-origin or destroyed frame
		}
	}
	return first
}

/**
 * The selection as it is now. Readium reports a selection on pointer-up only, but Android's
 * selection handles (and Shift+arrows) change it without one, so what the toolbar captured
 * can lag behind, e.g. the first word of a long-press after the handles widened it. Rebuilt
 * from the frame's live DOM selection: the captured locator keeps its place in the book and
 * gets the live quote (with the text around it, which is what finds it again).
 */
export function liveFrameSelection(
	container: HTMLDivElement | null,
	captured: AnnotationSelection,
): AnnotationSelection {
	const live = findFrameSelection(container)
	if (!container || !live) return captured
	const text = live.range.toString()
	if (!text.trim() || text === captured.text) return captured
	const context = extractSelectionContext(container)
	return {
		text,
		rect: selectionViewportRect(container) ?? captured.rect,
		locator: {
			...captured.locator,
			text: {
				highlight: text,
				before: context?.before ?? captured.locator.text?.before ?? null,
				after: context?.after ?? captured.locator.text?.after ?? null,
			},
		},
	}
}
