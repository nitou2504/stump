import type { ReaderLocator } from '../../annotations/types'

/**
 * A selection carried over a page turn (EPUB, paginated): the reader keeps where the first
 * selection started, turns the page, and the next selection says where the passage ends.
 * Both are in the same chapter document (the pages of a chapter are columns of one
 * document), so they make one range and one annotation.
 */
export type ExtendStart = {
	document: Document
	range: Range
	locator: ReaderLocator
}

/** The range from the start of `first` to the end of `second`, whichever comes first */
export function spanRanges(document: Document, first: Range, second: Range): Range {
	const startsFirst = first.compareBoundaryPoints(Range.START_TO_START, second) <= 0
	const from = startsFirst ? first : second
	const to = startsFirst ? second : first
	const range = document.createRange()
	range.setStart(from.startContainer, from.startOffset)
	// the later of the two ends (the first selection may already reach past the second)
	if (from.compareBoundaryPoints(Range.END_TO_END, to) > 0) {
		range.setEnd(from.endContainer, from.endOffset)
	} else {
		range.setEnd(to.endContainer, to.endOffset)
	}
	return range
}

/**
 * One locator for the whole passage: placed where it starts (its position and the text just
 * before it), quoting all of it, with the text just after its end. Readium finds the passage
 * again by that quote, across the page boundary.
 */
export function spanLocators(
	start: ReaderLocator,
	end: ReaderLocator,
	passage: string,
	{ startIsFirst = true }: { startIsFirst?: boolean } = {},
): ReaderLocator {
	const first = startIsFirst ? start : end
	const last = startIsFirst ? end : start
	return {
		...first,
		text: {
			before: first.text?.before ?? null,
			highlight: passage,
			after: last.text?.after ?? null,
		},
	}
}

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
