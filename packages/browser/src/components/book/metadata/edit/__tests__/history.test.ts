import { historyLabel, isListField, listDiff, listItems, saveDestination } from '../history'
import { describeFields } from '../outcome'

describe('edit history helpers', () => {
	it('diffs list values the way they are stored', () => {
		expect(listItems('Philosophy, Biography & Memoir,  ')).toEqual([
			'Philosophy',
			'Biography & Memoir',
		])
		expect(
			listDiff(
				'Physics & Astronomy, Physics & Astronomy: Astronomy, Level: General Reader',
				'Philosophy, Level: General Reader',
			),
		).toEqual({
			added: ['Philosophy'],
			removed: ['Physics & Astronomy', 'Physics & Astronomy: Astronomy'],
		})
		expect(listDiff(null, 'Robert Pantano')).toEqual({ added: ['Robert Pantano'], removed: [] })
		expect(isListField('genres')).toBe(true)
		expect(isListField('title')).toBe(false)
	})

	it('names fields and where a save went', () => {
		expect(historyLabel('writers')).toBe('authors')
		expect(historyLabel('month')).toBe('month')
		expect(describeFields(['genres', 'writers', 'month', 'day'])).toBe('genres, authors, date')
		expect(saveDestination([{ writtenToFile: true }, { writtenToFile: true }])).toBe('file')
		expect(saveDestination([{ writtenToFile: false }])).toBe('stump')
		expect(saveDestination([{ writtenToFile: true }, { writtenToFile: false }])).toBe('both')
	})
})
