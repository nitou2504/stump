import { render, screen } from '@testing-library/react'

import ReadingListGridItem from '../ReadingListGridItem'

vi.mock('@/hooks/useTranslate', () => import('./translations'))
vi.mock('@/paths', () => ({
	usePaths: () => ({ readingList: (id: string) => `/reading-lists/${id}` }),
}))
// the card is shown as its title and the chips on it
vi.mock('@/components/collection', () => ({
	CollectionGridItem: ({
		title,
		stats,
	}: {
		title: string
		stats: { key: string; value: number | string; label: string }[]
	}) => (
		<div>
			<p>{title}</p>
			<ul>
				{stats.map((stat) => (
					<li key={stat.key} title={stat.label}>
						{stat.value}
					</li>
				))}
			</ul>
		</div>
	),
}))

const list = (overrides: Record<string, unknown> = {}) =>
	({
		id: 'list',
		name: 'Recs',
		visibility: 'PRIVATE',
		canEdit: true,
		bookCount: 3,
		creator: { id: 'ana', username: 'ana' },
		books: [],
		...overrides,
	}) as never

const chips = () => screen.getAllByRole('listitem').map((chip) => chip.textContent)

describe('ReadingListGridItem', () => {
	it('shows only the number of books on a private list', () => {
		render(<ReadingListGridItem data={list()} getLayoutNumber={() => undefined} />)
		expect(chips()).toEqual(['3'])
	})

	it("shows a globe saying Public on the user's own public list", () => {
		render(
			<ReadingListGridItem
				data={list({ visibility: 'PUBLIC' })}
				getLayoutNumber={() => undefined}
			/>,
		)
		expect(chips()).toEqual(['3', 'readingLists.public'])
	})

	it("shows a globe with the name of the person who made a public list of someone else's", () => {
		render(
			<ReadingListGridItem
				data={list({ visibility: 'PUBLIC', canEdit: false })}
				getLayoutNumber={() => undefined}
			/>,
		)
		expect(chips()).toEqual(['3', 'ana'])
	})
})
