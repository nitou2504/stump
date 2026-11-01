import { packagePathFromHref } from '../readium/locator'

/** A book's note, ready for the reader's note popup */
export type NoteContent = {
	/** Sanitised HTML: text formatting and images only (no links, scripts, styles or ids) */
	html: string
	/** The note's plain text */
	text: string
}

const EPUB_NS = 'http://www.idpf.org/2007/ops'

/** `epub:type` values (EPUB 3 structural semantics) that mark a note or a list of notes */
const NOTE_TYPES = new Set([
	'footnote',
	'footnotes',
	'endnote',
	'endnotes',
	'rearnote',
	'rearnotes',
	'note',
])
/** The ARIA DPUB roles for the same */
const NOTE_ROLES = new Set(['doc-footnote', 'doc-endnote', 'doc-endnotes', 'doc-rearnote'])

/** A note's whole block, when its id sits on an inline element (an empty anchor, a span) */
const BLOCK_TAGS = new Set([
	'p',
	'div',
	'li',
	'aside',
	'section',
	'dd',
	'dt',
	'blockquote',
	'td',
	'footer',
	'article',
])
/** An inline target with at least this much text is the note itself */
const INLINE_NOTE_MIN_CHARS = 20

/** Kept as they are (formatting) */
const KEEP_TAGS = new Set([
	'p',
	'div',
	'span',
	'em',
	'i',
	'b',
	'strong',
	'sup',
	'sub',
	'br',
	'small',
	'cite',
	'q',
	'blockquote',
	'ul',
	'ol',
	'li',
	'dl',
	'dt',
	'dd',
	'code',
	'abbr',
	'u',
	's',
	'pre',
	'hr',
	'img',
	'h1',
	'h2',
	'h3',
	'h4',
	'h5',
	'h6',
	'table',
	'thead',
	'tbody',
	'tr',
	'td',
	'th',
	'figure',
	'figcaption',
	'var',
	'dfn',
	'mark',
])
/** Sectioning elements become plain blocks */
const RENAMED_TAGS: Record<string, string> = {
	aside: 'div',
	section: 'div',
	article: 'div',
	footer: 'div',
	header: 'div',
}
/** Dropped with everything inside them */
const DROPPED_TAGS = new Set([
	'script',
	'style',
	'link',
	'meta',
	'head',
	'title',
	'iframe',
	'frame',
	'object',
	'embed',
	'svg',
	'math',
	'noscript',
	'template',
	'audio',
	'video',
	'canvas',
	'form',
	'input',
	'button',
	'select',
	'textarea',
])
/** Text of the links that lead from a note back to its reference */
const BACK_LINK_TEXT = /^(<<|«|<|↩\uFE0E?|↑|⤴|\^|\[?\s*(back|return|volver|regresar)\s*\]?)$/i

const localName = (element: Element) => element.localName.toLowerCase()

/** An element's `epub:type` values, lower case */
export function epubTypesOf(element: Element): string[] {
	const value = element.getAttributeNS(EPUB_NS, 'type') || element.getAttribute('epub:type') || ''
	return value.toLowerCase().split(/\s+/).filter(Boolean)
}

/** An element's ARIA roles, lower case */
export function rolesOf(element: Element): string[] {
	return (element.getAttribute('role') ?? '').toLowerCase().split(/\s+/).filter(Boolean)
}

/** Whether the element, or one of its two closest ancestors, is marked as a note */
export function isNoteElement(element: Element | null): boolean {
	let current: Element | null = element
	for (let level = 0; current && level < 3; level++) {
		if (
			epubTypesOf(current).some((type) => NOTE_TYPES.has(type)) ||
			rolesOf(current).some((role) => NOTE_ROLES.has(role))
		) {
			return true
		}
		current = current.parentElement
	}
	return false
}

/** The element a link's fragment points at: by id, or by the old `name` anchors */
export function findTargetElement(doc: Document, id: string): Element | null {
	const byId = doc.getElementById(id)
	if (byId) return byId
	return (
		Array.from(doc.querySelectorAll('[name]')).find((el) => el.getAttribute('name') === id) ?? null
	)
}

/**
 * The elements that make up the note an id points at: the element itself when it is a
 * block (`<p id="nt1">`, `<aside id="fn1">`), the block around it when the id is on a small
 * inline element (`<p><a id="fn1"/>1. The note…</p>`), and a definition list's term with
 * its definition.
 */
export function noteElements(target: Element): Element[] {
	const name = localName(target)
	if (name === 'dt') {
		const definition = target.nextElementSibling
		return definition && localName(definition) === 'dd' ? [target, definition] : [target]
	}
	if (BLOCK_TAGS.has(name)) return [target]

	const text = target.textContent?.trim() ?? ''
	if (text.length >= INLINE_NOTE_MIN_CHARS) return [target]

	let block: Element | null = target.parentElement
	while (block && !BLOCK_TAGS.has(localName(block))) {
		if (localName(block) === 'body') return [target]
		block = block.parentElement
	}
	return block ? [block] : [target]
}

type SanitizeContext = {
	/** The note's resource URL, to resolve images */
	baseHref: string
	/** The package path of the page holding the reference, whose back-links are left out */
	sourcePath?: string
	/** The reference link's id, whose back-links are left out */
	refId?: string
}

function isBackLink(anchor: Element, context: SanitizeContext): boolean {
	if (rolesOf(anchor).includes('doc-backlink') || epubTypesOf(anchor).includes('backlink')) {
		return true
	}
	if (BACK_LINK_TEXT.test(anchor.textContent?.trim() ?? '')) return true

	const href = anchor.getAttribute('href')
	if (!href) return false
	let url: URL
	try {
		url = new URL(href, context.baseHref)
	} catch {
		return false
	}
	if (context.refId && url.hash === `#${context.refId}`) return true
	// A link to a place in the page the reader came from leads back to the reference
	return !!context.sourcePath && !!url.hash && packagePathFromHref(url.href) === context.sourcePath
}

function safeImageSource(src: string | null, baseHref: string): string | null {
	if (!src) return null
	try {
		const url = new URL(src, baseHref)
		if (url.protocol === 'http:' || url.protocol === 'https:') return url.href
		if (url.protocol === 'data:' && /^data:image\/(png|jpe?g|gif|webp)/i.test(url.href)) {
			return url.href
		}
	} catch {
		// unresolvable: dropped
	}
	return null
}

function copySanitized(source: Node, out: Element, context: SanitizeContext) {
	const doc = out.ownerDocument
	for (const child of Array.from(source.childNodes)) {
		if (child.nodeType === child.TEXT_NODE) {
			out.appendChild(doc.createTextNode(child.textContent ?? ''))
			continue
		}
		if (child.nodeType !== child.ELEMENT_NODE) continue

		const element = child as Element
		const name = localName(element)
		if (DROPPED_TAGS.has(name)) continue

		if (name === 'a') {
			// Links are not followed from the popup: keep their text, drop the way back
			if (!isBackLink(element, context)) copySanitized(element, out, context)
			continue
		}

		const tag = RENAMED_TAGS[name] ?? (KEEP_TAGS.has(name) ? name : null)
		if (!tag) {
			copySanitized(element, out, context)
			continue
		}

		if (tag === 'img') {
			const src = safeImageSource(element.getAttribute('src'), context.baseHref)
			if (!src) continue
			const image = doc.createElement('img')
			image.setAttribute('src', src)
			image.setAttribute('alt', element.getAttribute('alt') ?? '')
			out.appendChild(image)
			continue
		}

		const copy = doc.createElement(tag)
		out.appendChild(copy)
		if (tag !== 'br' && tag !== 'hr') copySanitized(element, copy, context)
	}
}

/**
 * The note an id points at in a book's page, for the note popup: its block (see
 * {@link noteElements}) copied into plain, safe HTML. Formatting and images stay; links
 * become text, and the links back to the reference are left out. `null` when there is no
 * such element or it has no text.
 */
export function extractNote(
	doc: Document,
	id: string,
	context: SanitizeContext,
): NoteContent | null {
	const target = findTargetElement(doc, id)
	if (!target) return null
	return sanitizeNote(noteElements(target), context)
}

/** A whole page's content, for links to a page the reader can't go to (no fragment) */
export function extractDocumentBody(doc: Document, context: SanitizeContext): NoteContent | null {
	return doc.body ? sanitizeNote([doc.body], context) : null
}

function sanitizeNote(elements: Element[], context: SanitizeContext): NoteContent | null {
	const outDoc = document.implementation.createHTMLDocument('')
	const container = outDoc.createElement('div')
	for (const element of elements) {
		// Each element's content, in a plain block (a `<p id>` note shows as a paragraph)
		const wrapper = outDoc.createElement('div')
		container.appendChild(wrapper)
		copySanitized(element, wrapper, context)
	}
	const text = plainText(container).replace(/\s+/g, ' ').trim()
	if (!text && !container.querySelector('img')) return null
	return { html: container.innerHTML, text }
}

/** An element's text, with blocks and line breaks kept apart by a space */
function plainText(node: Node): string {
	if (node.nodeType === node.TEXT_NODE) return node.textContent ?? ''
	if (node.nodeType !== node.ELEMENT_NODE) return ''
	const name = localName(node as Element)
	const inner = Array.from(node.childNodes).map(plainText).join('')
	return name === 'br' || BLOCK_TAGS.has(name) || /^h[1-6]$/.test(name) ? ` ${inner} ` : inner
}

/**
 * Parse a book page fetched from the server: as XHTML (what EPUB pages are), falling back
 * to HTML for the books whose pages aren't well-formed
 */
export function parseBookPage(markup: string): Document {
	const parser = new DOMParser()
	const xhtml = parser.parseFromString(markup, 'application/xhtml+xml')
	if (!xhtml.getElementsByTagName('parsererror').length) return xhtml
	return parser.parseFromString(markup, 'text/html')
}
