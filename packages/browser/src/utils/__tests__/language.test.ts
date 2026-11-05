import { bookLanguage, bookLanguageCode, languageName } from '../language'

describe('language', () => {
	it('reduces stored values to a language code', () => {
		expect(bookLanguageCode('es')).toBe('es')
		expect(bookLanguageCode(' ES ')).toBe('es')
		expect(bookLanguageCode('es-ES')).toBe('es')
		expect(bookLanguageCode('en_US')).toBe('en')
		expect(bookLanguageCode('spa')).toBe('es')
		expect(bookLanguageCode('eng')).toBe('en')
		expect(bookLanguageCode('jpn')).toBe('ja')
		expect(bookLanguageCode('ger')).toBe('de')
		// no two-letter code: the three-letter one
		expect(bookLanguageCode('ast')).toBe('ast')
	})

	it('shows nothing for empty or unreadable values', () => {
		expect(bookLanguageCode(null)).toBeNull()
		expect(bookLanguageCode(undefined)).toBeNull()
		expect(bookLanguageCode('  ')).toBeNull()
		expect(bookLanguageCode('und')).toBeNull()
		expect(bookLanguageCode('x')).toBeNull()
		expect(bookLanguageCode('English')).toBeNull()
		expect(bookLanguage('')).toBeNull()
	})

	it('names the language in the reader’s language', () => {
		expect(languageName('es', 'en-US')).toBe('Spanish')
		expect(languageName('en', 'es-ES')).toBe('Inglés')
		expect(bookLanguage('spa', 'en-US')).toEqual({ label: 'ES', name: 'Spanish' })
		expect(bookLanguage('ja', 'en-US')).toEqual({ label: 'JA', name: 'Japanese' })
	})
})
