import type { AnnotationStyle } from '@stump/graphql'

import { ANNOTATION_STYLES, BOOKMARK_SHORTCUT, HELP_SHORTCUT, NOTE_SHORTCUT } from './styles'

/** What a key press asks the reader to do */
export type ReaderShortcutAction =
	| { type: 'style'; style: AnnotationStyle }
	| { type: 'note' }
	| { type: 'delete' }
	| { type: 'bookmark' }
	| { type: 'help' }
	| { type: 'dismiss' }

type ShortcutContext = {
	/** Text is selected, or an existing annotation is selected (its toolbar is open) */
	hasTarget: boolean
	/** The selected thing is an existing annotation, which can be deleted */
	targetIsAnnotation: boolean
}

type KeyLike = Pick<KeyboardEvent, 'key' | 'ctrlKey' | 'metaKey' | 'altKey'>

/**
 * Maps a key press to a reader action, or `null` to leave it alone. Plain letters only
 * (any Ctrl/Cmd/Alt combination is someone else's shortcut):
 *
 * - with text or an annotation selected: `h` highlight, `u` underline, `s` strikethrough,
 *   `i` invert, `n` note, and for an annotation `Delete`/`Backspace` removes it
 * - anytime: `b` toggles a bookmark on the current page, `?` shows the shortcuts
 * - `Escape` dismisses the selection toolbar
 */
export function resolveReaderShortcut(
	event: KeyLike,
	{ hasTarget, targetIsAnnotation }: ShortcutContext,
): ReaderShortcutAction | null {
	if (event.ctrlKey || event.metaKey || event.altKey) return null

	const key = event.key.length === 1 ? event.key.toLowerCase() : event.key

	if (key === HELP_SHORTCUT) return { type: 'help' }
	if (key === BOOKMARK_SHORTCUT) return { type: 'bookmark' }

	if (!hasTarget) return null

	if (key === 'Escape') return { type: 'dismiss' }
	if (key === NOTE_SHORTCUT) return { type: 'note' }
	if (targetIsAnnotation && (key === 'Delete' || key === 'Backspace')) {
		return { type: 'delete' }
	}

	const style = ANNOTATION_STYLES.find((meta) => meta.shortcut === key)
	return style ? { type: 'style', style: style.style } : null
}

/** Whether a keydown target is an editable surface that should swallow reader shortcuts. */
export function isEditableTarget(target: EventTarget | null): boolean {
	if (!target || typeof (target as Element).closest !== 'function') return false
	const element = target as HTMLElement
	const tag = element.tagName
	if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return true
	if (element.isContentEditable) return true
	return !!element.closest('[role="dialog"]')
}

/**
 * Re-dispatches key presses made inside a same-origin reader frame (where the focus is
 * after selecting text in it) on the host document, so the reader's shortcuts, search
 * (Ctrl/Cmd+K) and arrow-key navigation work wherever the focus is. Returns a function
 * that stops forwarding.
 *
 * Key presses aimed at an editable element inside the book (a form in an EPUB) are not
 * forwarded. If the host handles the forwarded event (`preventDefault`), the original is
 * prevented too, e.g. so Space or arrows don't also scroll the frame.
 */
export function forwardFrameKeyEvents(frameDocument: Document, host: Document): () => void {
	const HostKeyboardEvent = host.defaultView?.KeyboardEvent ?? KeyboardEvent
	const forward = (event: KeyboardEvent) => {
		if (isEditableTarget(event.target)) return

		const forwarded = new HostKeyboardEvent('keydown', {
			key: event.key,
			code: event.code,
			ctrlKey: event.ctrlKey,
			metaKey: event.metaKey,
			altKey: event.altKey,
			shiftKey: event.shiftKey,
			repeat: event.repeat,
			bubbles: true,
			cancelable: true,
		})
		const notPrevented = host.dispatchEvent(forwarded)
		if (!notPrevented) {
			event.preventDefault()
		}
	}

	frameDocument.addEventListener('keydown', forward)
	return () => frameDocument.removeEventListener('keydown', forward)
}
