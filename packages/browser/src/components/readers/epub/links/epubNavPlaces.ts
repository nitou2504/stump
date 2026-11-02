import { Locator, LocatorLocations, LocatorText } from '@readium/shared'

import { FAR_JUMP_PAGES } from '../../shared/navHistory'
import { hrefsMatch } from '../readium/locator'

/**
 * A place in an EPUB for the reader's back and forward (see `shared/navHistory`): a Readium
 * locator as plain data, so it can be kept in session storage.
 */
export type EpubPlace = {
	href: string
	type?: string
	title?: string
	locations?: {
		fragments?: string[]
		progression?: number
		totalProgression?: number
		position?: number
		[key: string]: unknown
	}
	text?: Record<string, unknown>
}

export function placeOfLocator(locator: Locator): EpubPlace {
	return locator.serialize() as EpubPlace
}

export function locatorOfPlace(place: EpubPlace): Locator {
	return new Locator({
		href: place.href,
		// a contents link's locator has no type, which Readium's own reading refuses
		type: place.type || 'application/xhtml+xml',
		title: place.title,
		locations: LocatorLocations.deserialize(place.locations ?? {}) ?? new LocatorLocations({}),
		text: LocatorText.deserialize(place.text),
	})
}

export function isEpubPlace(value: unknown): value is EpubPlace {
	return (
		!!value && typeof value === 'object' && typeof (value as { href?: unknown }).href === 'string'
	)
}

/** The same spot: the same page of the book, or (without positions) nearly the same progression */
export function sameEpubPlace(a: EpubPlace, b: EpubPlace): boolean {
	if (!hrefsMatch(a.href, b.href)) return false
	const aPosition = a.locations?.position
	const bPosition = b.locations?.position
	if (aPosition != null && bPosition != null) return aPosition === bPosition
	return Math.abs((a.locations?.progression ?? 0) - (b.locations?.progression ?? 0)) < 0.01
}

/**
 * Whether going from `from` to `to` is a jump worth remembering: into another page of the
 * book's files (a chapter, as a rule) or more than a few locations away (about a page each)
 */
export function isFarEpubJump(
	from: { href: string; position?: number | null },
	to: { href: string; position?: number | null },
): boolean {
	if (!hrefsMatch(from.href, to.href)) return true
	if (from.position == null || to.position == null) return false
	return Math.abs(to.position - from.position) > FAR_JUMP_PAGES
}

/** How a place reads in "Back to …": its chapter and where it is in the book */
export function epubPlaceLabel(chapter: string | undefined, totalProgression?: number): string {
	const percent =
		totalProgression != null && Number.isFinite(totalProgression)
			? `${Math.round(Math.min(1, Math.max(0, totalProgression)) * 100)}%`
			: undefined
	return [chapter?.trim() || undefined, percent].filter(Boolean).join(' · ')
}
