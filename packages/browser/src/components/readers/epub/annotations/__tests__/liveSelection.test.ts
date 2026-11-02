import type { AnnotationSelection } from '../../../annotations/types'
import { liveFrameSelection } from '../liveSelection'

/** A reader container with one same-origin frame holding the chapter */
function readerWith(html: string): { container: HTMLDivElement; frameDocument: Document } {
	const container = document.createElement('div')
	const frame = document.createElement('iframe')
	container.appendChild(frame)
	document.body.appendChild(container)
	const frameDocument = frame.contentDocument!
	frameDocument.body.innerHTML = html
	return { container, frameDocument }
}

/** Selects `text` in the frame, like a long-press and then the handles would */
function select(frameDocument: Document, text: string) {
	const walker = frameDocument.createTreeWalker(frameDocument.body, NodeFilter.SHOW_TEXT)
	let node = walker.nextNode()
	while (node) {
		const index = node.textContent?.indexOf(text) ?? -1
		if (index >= 0) {
			const range = frameDocument.createRange()
			range.setStart(node, index)
			range.setEnd(node, index + text.length)
			const selection = frameDocument.getSelection()!
			selection.removeAllRanges()
			selection.addRange(range)
			return
		}
		node = walker.nextNode()
	}
	throw new Error(`no ${text}`)
}

const captured = (text: string): AnnotationSelection => ({
	text,
	rect: { x: 10, y: 20, width: 30, height: 12 },
	locator: {
		href: 'OEBPS/chapter1.xhtml',
		type: 'application/xhtml+xml',
		locations: { progression: 0.25, position: 7 },
		text: { highlight: text, before: null, after: null },
	},
})

afterEach(() => {
	document.body.innerHTML = ''
})

describe('liveFrameSelection', () => {
	it('follows the handles: the extended selection replaces the first word', () => {
		const { container, frameDocument } = readerWith(
			'<p>It was a bright cold day in April, and the clocks were striking thirteen.</p>',
		)
		// a long-press selected one word, then the handles widened it
		select(frameDocument, 'bright')
		const first = liveFrameSelection(container, captured('bright'))
		select(frameDocument, 'bright cold day in April')

		const live = liveFrameSelection(container, first)
		expect(live.text).toBe('bright cold day in April')
		expect(live.locator.text?.highlight).toBe('bright cold day in April')
		// the text around it, which finds the passage again
		expect(live.locator.text?.before).toBe('It was a ')
		expect(live.locator.text?.after?.startsWith(', and the clocks')).toBe(true)
		// its place in the book is kept
		expect(live.locator.href).toBe('OEBPS/chapter1.xhtml')
		expect(live.locator.locations?.position).toBe(7)
	})

	it('keeps what was captured when nothing changed or nothing is selected', () => {
		const { container, frameDocument } = readerWith('<p>It was a bright cold day in April.</p>')
		select(frameDocument, 'bright')
		const selection = captured('bright')
		expect(liveFrameSelection(container, selection)).toBe(selection)

		frameDocument.getSelection()!.removeAllRanges()
		expect(liveFrameSelection(container, selection)).toBe(selection)
		expect(liveFrameSelection(null, selection)).toBe(selection)
	})
})
