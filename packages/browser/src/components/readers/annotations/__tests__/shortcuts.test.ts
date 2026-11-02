import { AnnotationStyle } from '@stump/graphql'

import { forwardFrameKeyEvents, isEditableTarget, resolveReaderShortcut } from '../shortcuts'

const key = (k: string, modifiers: Partial<KeyboardEvent> = {}) => ({
	key: k,
	ctrlKey: false,
	metaKey: false,
	altKey: false,
	...modifiers,
})

const SELECTION = { hasTarget: true, targetIsAnnotation: false }
const ANNOTATION = { hasTarget: true, targetIsAnnotation: true }
const NOTHING = { hasTarget: false, targetIsAnnotation: false }

describe('resolveReaderShortcut', () => {
	it('maps h/u/s/i to the styles and n to a note when something is selected', () => {
		expect(resolveReaderShortcut(key('h'), SELECTION)).toEqual({
			type: 'style',
			style: AnnotationStyle.Highlight,
		})
		expect(resolveReaderShortcut(key('u'), SELECTION)).toEqual({
			type: 'style',
			style: AnnotationStyle.Underline,
		})
		expect(resolveReaderShortcut(key('S'), ANNOTATION)).toEqual({
			type: 'style',
			style: AnnotationStyle.Strikethrough,
		})
		expect(resolveReaderShortcut(key('i'), SELECTION)).toEqual({
			type: 'style',
			style: AnnotationStyle.Invert,
		})
		expect(resolveReaderShortcut(key('n'), SELECTION)).toEqual({ type: 'note' })
		expect(resolveReaderShortcut(key('Escape'), SELECTION)).toEqual({ type: 'dismiss' })
	})

	it('ignores annotation keys when nothing is selected', () => {
		for (const k of ['h', 'u', 's', 'i', 'n', 'Delete', 'Escape']) {
			expect(resolveReaderShortcut(key(k), NOTHING)).toBeNull()
		}
	})

	it('deletes only an existing annotation', () => {
		expect(resolveReaderShortcut(key('Delete'), ANNOTATION)).toEqual({ type: 'delete' })
		expect(resolveReaderShortcut(key('Backspace'), ANNOTATION)).toEqual({ type: 'delete' })
		expect(resolveReaderShortcut(key('Delete'), SELECTION)).toBeNull()
	})

	it('toggles a bookmark and shows the help anytime', () => {
		expect(resolveReaderShortcut(key('b'), NOTHING)).toEqual({ type: 'bookmark' })
		expect(resolveReaderShortcut(key('?', { shiftKey: true }), NOTHING)).toEqual({ type: 'help' })
	})

	it('searches with Ctrl/Cmd+F or / instead of the browser, and switches day/night with d', () => {
		expect(resolveReaderShortcut(key('f', { ctrlKey: true }), NOTHING)).toEqual({ type: 'search' })
		expect(resolveReaderShortcut(key('f', { metaKey: true }), SELECTION)).toEqual({
			type: 'search',
		})
		expect(resolveReaderShortcut(key('/'), NOTHING)).toEqual({ type: 'search' })
		expect(resolveReaderShortcut(key('d'), NOTHING)).toEqual({ type: 'dayNight' })
	})

	it('reads real keyboard events (their fields are getters)', () => {
		const event = new KeyboardEvent('keydown', { key: 'f', ctrlKey: true })
		expect(resolveReaderShortcut(event, NOTHING)).toEqual({ type: 'search' })
		expect(resolveReaderShortcut(new KeyboardEvent('keydown', { key: '/' }), NOTHING)).toEqual({
			type: 'search',
		})
	})

	it('copies the selected text or annotation with c', () => {
		expect(resolveReaderShortcut(key('c'), SELECTION)).toEqual({ type: 'copy' })
		expect(resolveReaderShortcut(key('c'), ANNOTATION)).toEqual({ type: 'copy' })
		expect(resolveReaderShortcut(key('c'), NOTHING)).toBeNull()
		// Ctrl+C stays the browser's copy
		expect(resolveReaderShortcut(key('c', { ctrlKey: true }), SELECTION)).toBeNull()
	})

	it('ignores key events without a key (autofill)', () => {
		expect(
			resolveReaderShortcut(
				{ key: undefined as unknown as string, ctrlKey: false, metaKey: false, altKey: false },
				NOTHING,
			),
		).toBeNull()
	})

	it("leaves combinations with Ctrl, Cmd or Alt alone (they're someone else's)", () => {
		expect(resolveReaderShortcut(key('b', { ctrlKey: true }), NOTHING)).toBeNull()
		expect(resolveReaderShortcut(key('h', { metaKey: true }), SELECTION)).toBeNull()
		expect(resolveReaderShortcut(key('s', { altKey: true }), SELECTION)).toBeNull()
	})
})

describe('isEditableTarget', () => {
	it('swallows inputs, textareas, editable content and anything in a dialog', () => {
		const input = document.createElement('input')
		const textarea = document.createElement('textarea')
		const editable = document.createElement('div')
		editable.contentEditable = 'true'
		const dialog = document.createElement('div')
		dialog.setAttribute('role', 'dialog')
		const inDialog = document.createElement('button')
		dialog.appendChild(inDialog)
		document.body.append(input, textarea, editable, dialog)

		expect(isEditableTarget(input)).toBe(true)
		expect(isEditableTarget(textarea)).toBe(true)
		expect(isEditableTarget(inDialog)).toBe(true)
		expect(isEditableTarget(document.body)).toBe(false)
		expect(isEditableTarget(window)).toBe(false)
		expect(isEditableTarget(null)).toBe(false)
	})
})

describe('forwardFrameKeyEvents', () => {
	it('re-dispatches frame key presses on the host and honours preventDefault', () => {
		const frameDocument = document.implementation.createHTMLDocument('frame')
		const received: string[] = []
		const onKeyDown = (event: KeyboardEvent) => {
			received.push(event.key)
			if (event.key === 'h') event.preventDefault()
		}
		window.addEventListener('keydown', onKeyDown)
		const stop = forwardFrameKeyEvents(frameDocument, document)

		const handled = new KeyboardEvent('keydown', { key: 'h', cancelable: true, bubbles: true })
		frameDocument.body.dispatchEvent(handled)
		const ignored = new KeyboardEvent('keydown', { key: 'x', cancelable: true, bubbles: true })
		frameDocument.body.dispatchEvent(ignored)

		expect(received).toEqual(['h', 'x'])
		expect(handled.defaultPrevented).toBe(true)
		expect(ignored.defaultPrevented).toBe(false)

		stop()
		frameDocument.body.dispatchEvent(new KeyboardEvent('keydown', { key: 'u', bubbles: true }))
		expect(received).toEqual(['h', 'x'])
		window.removeEventListener('keydown', onKeyDown)
	})

	it("doesn't forward typing in an input inside the frame", () => {
		const frameDocument = document.implementation.createHTMLDocument('frame')
		const input = frameDocument.createElement('input')
		frameDocument.body.appendChild(input)
		const onKeyDown = vi.fn()
		window.addEventListener('keydown', onKeyDown)
		const stop = forwardFrameKeyEvents(frameDocument, document)

		input.dispatchEvent(new KeyboardEvent('keydown', { key: 'h', bubbles: true }))
		expect(onKeyDown).not.toHaveBeenCalled()

		stop()
		window.removeEventListener('keydown', onKeyDown)
	})
})
