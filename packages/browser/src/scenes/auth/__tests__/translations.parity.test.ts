import en from '../../../../../i18n/src/locales/en-US.json'
import es from '../../../../../i18n/src/locales/es-ES.json'

/** The words of the public sign-in and join screens exist in both languages, with the same holes */
describe('the sign-in and join screens in Spanish', () => {
	const keys = (value: unknown, prefix = ''): string[] =>
		value && typeof value === 'object'
			? Object.entries(value).flatMap(([key, inner]) => keys(inner, `${prefix}${key}.`))
			: [prefix.slice(0, -1)]
	const text = (tree: unknown, path: string) =>
		path.split('.').reduce<unknown>((node, key) => (node as Record<string, unknown>)[key], tree)

	const english = en.webApp.authScene
	const spanish = es.webApp.authScene

	it('has every key in both languages', () => {
		for (const part of ['public', 'join'] as const) {
			expect(keys(spanish[part]).sort(), part).toEqual(keys(english[part]).sort())
		}
	})

	it('says something different in each, and keeps the same placeholders', () => {
		for (const part of ['public', 'join'] as const) {
			for (const key of keys(english[part])) {
				const inEnglish = String(text(english[part], key))
				const inSpanish = String(text(spanish[part], key))
				expect(inEnglish.length, `${part}.${key}`).toBeGreaterThan(0)
				expect(inSpanish.length, `${part}.${key}`).toBeGreaterThan(0)
				// a label or two may be the same word in both; the sentences are not
				if (inEnglish.length > 12) expect(inSpanish, `${part}.${key}`).not.toBe(inEnglish)
				expect(inSpanish.match(/\{\{\w+\}\}/g) ?? [], `${part}.${key}`).toEqual(
					inEnglish.match(/\{\{\w+\}\}/g) ?? [],
				)
			}
		}
	})

	it('keeps the short wording the owner asked for', () => {
		expect(english.public.wrongCredentials).toBe('Wrong username or password.')
		expect(english.public.forgot).toBe('Forgot your password? Ask the owner.')
		// no screen text about invite links on the login screen (people who hold one just open it)
		expect(JSON.stringify(english.public).toLowerCase()).not.toContain('invite')
		expect(JSON.stringify(spanish.public).toLowerCase()).not.toContain('invitaci')
	})
})
