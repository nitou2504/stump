import { render, screen } from '@testing-library/react'

import SmartListGridItem from '../SmartListGridItem'
import { setup } from './support'

vi.mock('@/hooks/useTranslate', () => import('./translations'))
vi.mock('@/paths', () => ({
	usePaths: () => ({ smartList: (id: string) => `/smart-lists/${id}` }),
}))
// the card is shown as its title and the chips on it
vi.mock('@/components/collection', () => ({
	CollectionGridItem: ({
		title,
		stats,
	}: {
		title: string
		stats: { key: string; value: number | string; unit?: string; label: string }[]
	}) => (
		<div>
			<p>{title}</p>
			<ul>
				{stats.map((stat) => (
					<li key={stat.key} title={stat.label}>
						{stat.unit ? `${stat.value} ${stat.unit}` : stat.value}
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
		creatorId: 'me',
		visibility: 'PRIVATE',
		creator: { id: 'me', username: 'me' },
		books: [],
		meta: { matchedBooks: 3, matchedSeries: 2, matchedLibraries: 1 },
		...overrides,
	}) as never

const chip = (key: string) =>
	screen.getAllByRole('listitem').find((item) => item.textContent === key)

function renderCard(data: never) {
	const { Wrapper } = setup({ answers: () => ({}) })
	return render(<SmartListGridItem data={data} getLayoutNumber={() => undefined} />, {
		wrapper: Wrapper,
	})
}

describe('SmartListGridItem', () => {
	it("says nothing about a private list of the user's own (it is the usual case)", () => {
		renderCard(list())

		expect(chip('userSmartListsScene.list.card.private')).toBeUndefined()
		expect(chip('userSmartListsScene.list.card.shared')).toBeUndefined()
	})

	it("says a list of the user's own that everyone sees is shared", () => {
		renderCard(list({ visibility: 'PUBLIC' }))

		expect(chip('userSmartListsScene.list.card.shared')).toHaveAttribute(
			'title',
			'userSmartListsScene.list.card.sharedHint',
		)
		expect(chip('userSmartListsScene.list.card.private')).toBeUndefined()
	})

	it('says who made the list of someone else', () => {
		renderCard(
			list({
				creatorId: 'ana',
				visibility: 'PUBLIC',
				creator: { id: 'ana', username: 'Ana' },
			}),
		)

		expect(chip('userSmartListsScene.list.card.by name=Ana')).toHaveAttribute(
			'title',
			'userSmartListsScene.list.card.byHint name=Ana',
		)
		// a list of someone else isn't labelled as the user's own
		expect(chip('userSmartListsScene.list.card.shared')).toBeUndefined()
		expect(chip('userSmartListsScene.list.card.private')).toBeUndefined()
	})

	it('counts what the list matches in words, never as bare numbers', () => {
		renderCard(list())

		expect(screen.getAllByRole('listitem').map((item) => item.textContent)).toEqual([
			'3 statChips.books count=3',
			'2 statChips.series count=2',
		])
	})

	it('counts the libraries only when the list spans more than one', () => {
		renderCard(list({ meta: { matchedBooks: 30, matchedSeries: 9, matchedLibraries: 2 } }))

		expect(screen.getAllByRole('listitem').map((item) => item.textContent)).toEqual([
			'30 statChips.books count=30',
			'9 statChips.series count=9',
			'2 statChips.libraries count=2',
		])
	})
})
