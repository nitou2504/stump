import { spanLocators, spanRanges } from '../extendSelection'

function documentWith(html: string): Document {
	const document = window.document.implementation.createHTMLDocument('chapter')
	document.body.innerHTML = html
	return document
}

function rangeOver(document: Document, text: string): Range {
	const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT)
	let node = walker.nextNode()
	while (node) {
		const index = node.textContent?.indexOf(text) ?? -1
		if (index >= 0) {
			const range = document.createRange()
			range.setStart(node, index)
			range.setEnd(node, index + text.length)
			return range
		}
		node = walker.nextNode()
	}
	throw new Error(`no ${text}`)
}

describe('extending a selection over a page turn', () => {
	const document = documentWith(
		'<p>It was a bright cold day in April,</p><p>and the clocks were striking thirteen.</p>',
	)

	it('spans from the first selection to where the passage ends on the next page', () => {
		const start = rangeOver(document, 'bright cold')
		const end = rangeOver(document, 'striking')
		expect(spanRanges(document, start, end).toString()).toBe(
			'bright cold day in April,and the clocks were striking',
		)
	})

	it('works whichever selection comes first in the chapter', () => {
		const later = rangeOver(document, 'thirteen')
		const earlier = rangeOver(document, 'April')
		expect(spanRanges(document, later, earlier).toString()).toBe(
			'April,and the clocks were striking thirteen',
		)
	})

	it('keeps the first end when the second selection is inside the first', () => {
		const outer = rangeOver(document, 'bright cold day')
		const inner = rangeOver(document, 'cold')
		expect(spanRanges(document, outer, inner).toString()).toBe('bright cold day')
	})

	it('makes one locator placed at the start, quoting the whole passage', () => {
		const start = {
			href: 'ch1.xhtml',
			type: 'application/xhtml+xml',
			locations: { progression: 0.2, position: 4 },
			text: { before: 'It was a ', highlight: 'bright cold', after: ' day' },
		}
		const end = {
			href: 'ch1.xhtml',
			type: 'application/xhtml+xml',
			locations: { progression: 0.6, position: 5 },
			text: { before: 'clocks were ', highlight: 'striking', after: ' thirteen.' },
		}
		expect(spanLocators(start, end, 'bright cold … striking')).toEqual({
			...start,
			text: { before: 'It was a ', highlight: 'bright cold … striking', after: ' thirteen.' },
		})
		expect(spanLocators(start, end, 'x', { startIsFirst: false }).locations).toEqual(end.locations)
	})
})
