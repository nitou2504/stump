import { inSentence } from '../permissions'

describe('A permission’s name in the middle of a sentence', () => {
	it('starts in lower case, as the words around it do', () => {
		expect(inSentence('Scan libraries')).toBe('scan libraries')
		expect(inSentence('Escanear bibliotecas')).toBe('escanear bibliotecas')
	})

	it('keeps a name that starts with an acronym, and one that is already lower case', () => {
		expect(inSentence('API keys')).toBe('API keys')
		expect(inSentence('manage users')).toBe('manage users')
	})
})
