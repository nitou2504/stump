import { Locator, LocatorLocations, LocatorText } from '@readium/shared'

import {
	epubPlaceLabel,
	isEpubPlace,
	isFarEpubJump,
	locatorOfPlace,
	placeOfLocator,
	sameEpubPlace,
} from '../epubNavPlaces'

const locator = (href: string, position?: number, progression?: number) =>
	new Locator({
		href,
		type: 'application/xhtml+xml',
		title: 'Chapter',
		locations: new LocatorLocations({ position, progression, totalProgression: 0.42 }),
		text: new LocatorText({ highlight: 'words' }),
	})

describe('EPUB places', () => {
	it('round-trips a locator through plain data (session storage)', () => {
		const original = locator('OEBPS/Text/ch3.xhtml', 120, 0.25)
		const stored = JSON.parse(JSON.stringify(placeOfLocator(original)))
		expect(isEpubPlace(stored)).toBe(true)
		const back = locatorOfPlace(stored)
		expect(back.href).toBe('OEBPS/Text/ch3.xhtml')
		expect(back.locations.position).toBe(120)
		expect(back.locations.progression).toBe(0.25)
		expect(back.text?.highlight).toBe('words')
	})

	it('reads a stored place without a type', () => {
		const back = locatorOfPlace({ href: 'OEBPS/Text/ch3.xhtml' })
		expect(back.type).toBe('application/xhtml+xml')
	})

	it('knows the same spot', () => {
		const a = placeOfLocator(locator('OEBPS/Text/ch3.xhtml', 120))
		expect(sameEpubPlace(a, placeOfLocator(locator('OEBPS/Text/ch3.xhtml#x', 120)))).toBe(true)
		expect(sameEpubPlace(a, placeOfLocator(locator('OEBPS/Text/ch3.xhtml', 121)))).toBe(false)
		expect(sameEpubPlace(a, placeOfLocator(locator('OEBPS/Text/ch4.xhtml', 120)))).toBe(false)
		// without positions: nearly the same progression
		expect(
			sameEpubPlace(
				{ href: 'a.xhtml', locations: { progression: 0.5 } },
				{
					href: 'a.xhtml',
					locations: { progression: 0.505 },
				},
			),
		).toBe(true)
	})

	it('counts a jump into another file or more than a few locations', () => {
		expect(isFarEpubJump({ href: 'ch3.xhtml', position: 120 }, { href: 'ch4.xhtml' })).toBe(true)
		expect(
			isFarEpubJump({ href: 'ch3.xhtml', position: 120 }, { href: 'ch3.xhtml', position: 118 }),
		).toBe(false)
		expect(
			isFarEpubJump({ href: 'ch3.xhtml', position: 120 }, { href: 'ch3.xhtml', position: 110 }),
		).toBe(true)
	})

	it('names a place by its chapter and its share of the book', () => {
		expect(epubPlaceLabel('Chapter 3', 0.124)).toBe('Chapter 3 · 12%')
		expect(epubPlaceLabel(undefined, 0.5)).toBe('50%')
		expect(epubPlaceLabel('  ', undefined)).toBe('')
	})
})
