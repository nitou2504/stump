import { detailsForm, editInput } from '../form'

const stored = {
	ageRating: 0,
	genres: [
		'Physics & Astronomy',
		'Physics & Astronomy: Relativity & Cosmology',
		'Level: General Reader',
		'Collection: Owned',
	],
	language: 'en',
	number: null,
	publisher: null,
	series: null,
	summary: 'A diary.\n',
	title: 'Notes from the End of Everything',
	writers: ['Robert Pantano'],
	year: 2020,
}

describe('edit details form', () => {
	it('sends nothing when nothing was touched', () => {
		const initial = detailsForm(stored)
		expect(editInput(initial, { ...initial })).toEqual({})
		// untouched text keeps its stray whitespace: no rewrite of the summary
		expect(initial.summary).toBe('A diary.\n')
	})

	it('sends only the edited fields, genres in the library order', () => {
		const initial = detailsForm(stored)
		const input = editInput(initial, {
			...initial,
			genres: ['Level: General Reader', 'Philosophy', 'Biography & Memoir: Memoir'],
			title: '  Notes from the End of Everything ',
		})
		expect(input).toEqual({
			genres: ['Philosophy', 'Biography & Memoir: Memoir', 'Level: General Reader'],
		})
	})

	it('clears the month and day with a new year, and the number with the series', () => {
		const initial = detailsForm({ ...stored, number: 2, series: 'Pursuit' })
		expect(editInput(initial, { ...initial, year: '2021' })).toEqual({
			day: null,
			month: null,
			year: 2021,
		})
		expect(editInput(initial, { ...initial, series: ' ' })).toEqual({ number: null, series: null })
		expect(editInput(initial, { ...initial, number: '2,5' })).toEqual({ number: 2.5 })
	})

	it('turns emptied fields into nulls and ages into numbers', () => {
		const initial = detailsForm(stored)
		expect(editInput(initial, { ...initial, ageRating: '18', language: '', summary: '' })).toEqual({
			ageRating: 18,
			language: null,
			summary: null,
		})
		expect(editInput(initial, { ...initial, ageRating: '' })).toEqual({ ageRating: null })
	})
})
