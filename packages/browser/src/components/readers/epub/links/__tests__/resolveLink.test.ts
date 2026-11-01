import { Locator, LocatorLocations } from '@readium/shared'

import {
	isExternalHref,
	isNoteLink,
	isShortReference,
	type LinkFacts,
	linkLooksLikeNote,
	resolvePublicationTarget,
	targetLocator,
} from '../resolveLink'

const BOOK = 'http://localhost:10801/api/v2/epub/book/resource/OEBPS/Text'
const chapter = `${BOOK}/chapter01.xhtml`
const notes = `${BOOK}/notas.xhtml`
const resourceHrefs = [chapter, notes, `${BOOK}/../Images/cover.jpg`]
// The positions list only covers the reading order's linear pages: not ePubLibre's notes
const positions = [
	new Locator({
		href: chapter,
		type: 'application/xhtml+xml',
		locations: new LocatorLocations({ position: 1, progression: 0, totalProgression: 0 }),
	}),
]

const facts = (overrides: Partial<LinkFacts> = {}): LinkFacts => ({
	text: '',
	epubTypes: [],
	roles: [],
	superscript: false,
	...overrides,
})

describe('resolvePublicationTarget', () => {
	it('finds a page out of the reading order, which the reader cannot go to', () => {
		const target = resolvePublicationTarget(`${notes}#nt1`, resourceHrefs, positions)
		expect(target).toMatchObject({
			resourceHref: notes,
			packagePath: 'OEBPS/Text/notas.xhtml',
			fragment: 'nt1',
			position: null,
		})
	})

	it('reads the `http:/host` URLs Readium hands over', () => {
		const target = resolvePublicationTarget(
			'http:/localhost:10801/api/v2/epub/book/resource/OEBPS/Text/chapter01.xhtml#rf1',
			resourceHrefs,
			positions,
		)
		expect(target?.resourceHref).toBe(chapter)
		expect(target?.fragment).toBe('rf1')
		expect(target?.position?.locations.position).toBe(1)
	})

	it('does not take a link to another site for a page with the same path', () => {
		expect(
			resolvePublicationTarget(
				'https://example.com/api/v2/epub/book/resource/OEBPS/Text/chapter01.xhtml',
				resourceHrefs,
				positions,
			),
		).toBeNull()
		expect(resolvePublicationTarget('https://example.com/', resourceHrefs, positions)).toBeNull()
	})
})

describe('targetLocator', () => {
	it('goes to the link id on a page in the reading order', () => {
		const target = resolvePublicationTarget(`${chapter}#rf1`, resourceHrefs, positions)!
		const locator = targetLocator(target)
		expect(locator?.href).toBe(chapter)
		expect(locator?.locations.fragments).toEqual(['rf1'])
		expect(locator?.locations.position).toBe(1)
	})

	it('has nowhere to go for a page out of the reading order', () => {
		const target = resolvePublicationTarget(`${notes}#nt1`, resourceHrefs, positions)!
		expect(targetLocator(target)).toBeNull()
	})
})

describe('isExternalHref', () => {
	it('opens web, mail and phone links outside the reader', () => {
		expect(isExternalHref('https://en.wikipedia.org/wiki/Note')).toBe(true)
		expect(isExternalHref('http:/example.com/page')).toBe(true)
		expect(isExternalHref('mailto:a@b.c')).toBe(true)
		expect(isExternalHref('notas.xhtml#nt1')).toBe(false)
	})
})

describe('note references', () => {
	it('recognises note numbers and marks', () => {
		for (const text of ['1', '[12]', '(3)', 'iv', '*', '†', 'a', '12a', '[1].']) {
			expect(isShortReference(text)).toBe(true)
		}
		for (const text of ['Notas', 'Chapter 1', '1. Miss Lonelyhearts', '<<']) {
			expect(isShortReference(text)).toBe(false)
		}
	})

	it('trusts EPUB 3 and ARIA note reference markup', () => {
		expect(linkLooksLikeNote(facts({ text: 'see the note', epubTypes: ['noteref'] }))).toBe(true)
		expect(linkLooksLikeNote(facts({ text: 'see the note', roles: ['doc-noteref'] }))).toBe(true)
	})

	it('takes a superscript number, or a number pointing at a note id, for a reference', () => {
		expect(linkLooksLikeNote(facts({ text: '[1]', superscript: true }))).toBe(true)
		expect(linkLooksLikeNote(facts({ text: '2' }), 'fn2')).toBe(true)
		expect(linkLooksLikeNote(facts({ text: '2' }), 'chapter2')).toBe(false)
		expect(linkLooksLikeNote(facts({ text: 'Chapter 1', superscript: true }))).toBe(false)
	})

	it('opens a note when the target is marked as one or cannot be gone to', () => {
		const doc = new DOMParser().parseFromString(
			'<html><body><aside role="doc-footnote" id="fn1"><p>A note.</p></aside><p id="p2">Text</p></body></html>',
			'text/html',
		)
		const plain = facts({ text: 'see this' })
		expect(
			isNoteLink(plain, { element: doc.getElementById('fn1'), navigable: true, fragment: 'fn1' }),
		).toBe(true)
		expect(
			isNoteLink(plain, { element: doc.getElementById('p2'), navigable: true, fragment: 'p2' }),
		).toBe(false)
		// e.g. ePubLibre's notes page, kept out of the reading order
		expect(isNoteLink(plain, { element: null, navigable: false, fragment: 'nt1' })).toBe(true)
	})

	it('moves the reader for an ordinary link, e.g. a table of contents', () => {
		expect(
			isNoteLink(facts({ text: '1. Miss Lonelyhearts, ayúdame' }), {
				element: null,
				navigable: true,
			}),
		).toBe(false)
	})
})
