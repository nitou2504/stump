import type { Locator } from '@readium/shared'

import { fragmentFromHref, hrefsMatch, packagePathFromHref, unmangleHref } from '../readium/locator'
import { isNoteElement } from './noteContent'

/** What a tapped link says about itself, read from the book's page */
export type LinkFacts = {
	/** The link's text */
	text: string
	/** Its `epub:type` values (`noteref` marks a note reference) */
	epubTypes: string[]
	/** Its ARIA roles (`doc-noteref` marks a note reference) */
	roles: string[]
	/** It is, contains or sits in a superscript, as note numbers usually do */
	superscript: boolean
}

/** A link inside the publication */
export type PublicationLinkTarget = {
	/** The resource's href in the manifest (absolute on the web) */
	resourceHref: string
	/** Its path inside the EPUB package, e.g. `OEBPS/Text/notas.xhtml` */
	packagePath: string
	/** The id in the resource the link points at */
	fragment?: string
	/**
	 * The resource's first position, when the reader can go there. Pages kept out of the
	 * reading order (`linear="no"`, e.g. ePubLibre's `notas.xhtml`) have none
	 */
	position: Locator | null
}

const isAbsoluteUrl = (href: string) => /^[a-z][a-z0-9+.-]*:/i.test(href)

function sameOrigin(a: string, b: string): boolean {
	try {
		return new URL(a).origin === new URL(b).origin
	} catch {
		return false
	}
}

/**
 * The publication resource a link points at, or `null` when it leads outside the book.
 * `resourceHrefs` are the manifest's reading order and resources; `positions` the
 * reader's positions list (only the pages in the reading order).
 */
export function resolvePublicationTarget(
	href: string,
	resourceHrefs: string[],
	positions: Locator[],
): PublicationLinkTarget | null {
	const fixed = unmangleHref(href)
	const resourceHref = resourceHrefs.find(
		(candidate) =>
			hrefsMatch(candidate, fixed) &&
			// An absolute link elsewhere must not match a resource by its path alone
			(!isAbsoluteUrl(fixed) || !isAbsoluteUrl(candidate) || sameOrigin(candidate, fixed)),
	)
	if (!resourceHref) return null

	return {
		resourceHref,
		packagePath: packagePathFromHref(resourceHref),
		fragment: fragmentFromHref(fixed),
		position: positions.find((position) => hrefsMatch(position.href, resourceHref)) ?? null,
	}
}

/** Where the reader goes for a link: the resource's position, at the link's id */
export function targetLocator(target: PublicationLinkTarget): Locator | null {
	if (!target.position) return null
	return target.fragment
		? target.position.copyWithLocations({ fragments: [target.fragment] })
		: target.position
}

/** Links that leave the book and open in a new tab */
export function isExternalHref(href: string): boolean {
	return /^(https?:\/\/|mailto:|tel:)/i.test(unmangleHref(href))
}

/** Note numbers and marks: `1`, `[12]`, `(3)`, `iv`, `*`, `†`, `a` */
const SHORT_REFERENCE = /^[[(]?\s*(\d{1,4}[a-z]?|[ivxlc]{1,6}|[*†‡§¶#]{1,3}|[a-z])\s*[\])]?\.?$/i

/** Whether a link's text is a note number or mark (see {@link SHORT_REFERENCE}) */
export function isShortReference(text: string): boolean {
	return SHORT_REFERENCE.test(text.trim())
}
/** Ids that name notes: `nt1`, `fn3`, `note-12`, `footnote_4`, `n10_0` */
const NOTE_ID = /^(nt|n|fn|ftn|note|footnote|endnote|en|rn|nota)[-_.]?\d+([-_.]?\d+)?[a-z]?$/i

/**
 * Whether a link, judged on its own, looks like a note reference: marked as one, or a
 * short number/mark that is a superscript or points at an id that names a note.
 */
export function linkLooksLikeNote(link: LinkFacts, fragment?: string): boolean {
	if (link.epubTypes.includes('noteref') || link.roles.includes('doc-noteref')) return true
	if (!isShortReference(link.text)) return false
	return link.superscript || (!!fragment && NOTE_ID.test(fragment))
}

/**
 * Whether a link opens a note in a popup rather than moving the reader, like KOReader does:
 * the link looks like a note reference, its target is marked as a note, or the target is
 * on a page the reader can't go to (out of the reading order, as ePubLibre's notes are).
 */
export function isNoteLink(
	link: LinkFacts,
	target: { element: Element | null; navigable: boolean; fragment?: string },
): boolean {
	if (linkLooksLikeNote(link, target.fragment)) return true
	if (target.element && isNoteElement(target.element)) return true
	return !target.navigable
}
