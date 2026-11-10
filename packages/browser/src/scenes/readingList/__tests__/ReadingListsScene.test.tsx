import '@/__mocks__/resizeObserver'

import { render, screen, within } from '@testing-library/react'

import ReadingListsScene from '../ReadingListsScene'
import { setup } from './support'

vi.mock('@/hooks/useTranslate', () => import('./translations'))
// the page's header and the cards are tested where they are; here only what the page lists
vi.mock('@/components/sharedLayout', () => ({
	CollectionSceneHeader: ({ title }: { title: string }) => <h1>{title}</h1>,
	HeaderIconButton: ({ label }: { label: string }) => <button>{label}</button>,
}))
vi.mock('../ReadingListGridItem', () => ({
	default: ({ data }: { data: { id: string; name: string } }) => (
		<a href={`/${data.id}`}>{data.name}</a>
	),
}))

const ana = { id: 'ana', username: 'ana', avatarUrl: null }
const ben = { id: 'ben', username: 'ben', avatarUrl: null }
const list = (
	id: string,
	name: string,
	creator: typeof ana | { id: string; username: string; avatarUrl: null },
	bookCount = 3,
) => ({ id, name, creatingUserId: creator.id, bookCount, creator })
const me = { id: 'me', username: 'me', avatarUrl: null }

function renderPage(nodes: ReturnType<typeof list>[]) {
	const { Wrapper } = setup({
		answers: (query) => {
			if (query.includes('query ReadingListsScene')) return { readingLists: { nodes } }
			throw new Error(`Unexpected query ${query}`)
		},
	})
	return render(<ReadingListsScene />, { wrapper: Wrapper })
}

describe('ReadingListsScene', () => {
	it("shows the user's lists, then the public lists of others grouped by who made them", async () => {
		renderPage([
			list('c', 'Ben picks', ben),
			list('b', 'Zebras', me),
			list('a', 'Ana recs', ana),
			list('d', 'Aardvarks', me),
			list('e', 'Ana classics', ana),
		])

		const mine = await screen.findByRole('region', { name: 'readingLists.sections.mine' })
		expect(
			within(mine)
				.getAllByRole('link')
				.map((link) => link.textContent),
		).toEqual(['Aardvarks', 'Zebras'])

		const others = screen.getByRole('region', { name: 'readingLists.sections.others' })
		// people in name order, each with their lists in name order
		expect(
			within(others)
				.getAllByRole('link')
				.map((link) => link.textContent),
		).toEqual(['Ana classics', 'Ana recs', 'Ben picks'])
		expect(within(others).getByText('ana')).toBeInTheDocument()
		expect(within(others).getByText('ben')).toBeInTheDocument()
	})

	it('leaves out a list of someone else that holds none of the books the user may read', async () => {
		renderPage([list('a', 'Ana hidden', ana, 0), list('b', 'Ana recs', ana, 2)])

		expect(await screen.findByText('Ana recs')).toBeInTheDocument()
		expect(screen.queryByText('Ana hidden')).not.toBeInTheDocument()
	})

	// it's the user's to fill
	it("shows an empty list of the user's own", async () => {
		renderPage([list('a', 'Nothing yet', me, 0)])
		expect(await screen.findByText('Nothing yet')).toBeInTheDocument()
	})

	it('has no section for others when nobody shared a list', async () => {
		renderPage([list('a', 'Mine', me)])

		expect(await screen.findByText('Mine')).toBeInTheDocument()
		expect(screen.queryByRole('region', { name: 'readingLists.sections.others' })).toBeNull()
	})

	it("invites the user to make a list when there is none, theirs or anyone's", async () => {
		renderPage([])

		expect(await screen.findByText('readingLists.emptyState.heading')).toBeInTheDocument()
		expect(screen.getAllByRole('button', { name: 'readingLists.newList' }).length).toBeGreaterThan(
			0,
		)
	})

	it("tells the user they haven't made a list yet when only others have", async () => {
		renderPage([list('a', 'Ana recs', ana)])

		expect(await screen.findByText('readingLists.emptyState.mine')).toBeInTheDocument()
		expect(screen.getByText('Ana recs')).toBeInTheDocument()
	})
})
