import { browserLocale } from '../browserLocale'

describe('browserLocale', () => {
	it('is Spanish when the first language of the browser is Spanish', () => {
		for (const languages of [
			['es'],
			['es-EC'],
			['es-ES', 'en-US'],
			['ES-mx', 'en'],
			[' es-419 '],
		]) {
			expect(browserLocale(languages), languages.join()).toBe('es-ES')
		}
	})

	it('is English for everybody else, and when there is nothing to go by', () => {
		for (const languages of [
			['en-US'],
			['en', 'es'], // only the first counts, as on the server's own pages
			['fr-FR', 'es'],
			['esperanto'],
			['est'], // Estonian is not Spanish
			[''],
			[],
			undefined,
		]) {
			expect(browserLocale(languages), String(languages)).toBe('en-US')
		}
	})
})
