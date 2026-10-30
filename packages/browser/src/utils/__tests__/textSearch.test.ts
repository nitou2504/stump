import { foldText, matchesSearch, searchWords } from '../textSearch'

describe('textSearch', () => {
	it('folds case and accents', () => {
		expect(foldText('Física CUÁNTICA Ñandú')).toBe('fisica cuantica nandu')
	})

	it('splits a query into unique words', () => {
		expect(searchWords('  Pearson   brown ')).toEqual(['pearson', 'brown'])
		expect(searchWords('Catrin_Brown brown')).toEqual(['catrin', 'brown'])
		expect(searchWords('¿Física?')).toEqual(['fisica'])
	})

	it('needs every word somewhere, in any order and field', () => {
		const texts = ['Chemistry: Higher Level', 'Catrin Brown', 'Pearson Baccalaureate']
		expect(matchesSearch('pearson brown', texts)).toBe(true)
		expect(matchesSearch('brown chemistry', texts)).toBe(true)
		expect(matchesSearch('brown physics', texts)).toBe(false)
		expect(matchesSearch('quimica', ['Asignatura: Química'])).toBe(true)
		expect(matchesSearch('  ', texts)).toBe(true)
	})

	it('matches the start of words only', () => {
		expect(matchesSearch('quim', ['Asignatura: Química'])).toBe(true)
		expect(matchesSearch('ib', ['Física (Diploma del IB)'])).toBe(true)
		expect(matchesSearch('ib', ['/biblioteca/libros'])).toBe(false)
		expect(matchesSearch('istry', ['Chemistry'])).toBe(false)
	})
})
