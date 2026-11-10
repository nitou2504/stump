import '@/__mocks__/resizeObserver'

import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import UserSmartListsScene from '../UserSmartListsScene'
import { sent, setup } from './support'

vi.mock('@/hooks/useTranslate', () => import('./translations'))
const toast = vi.hoisted(() => ({ success: vi.fn(), error: vi.fn() }))
vi.mock('sonner', () => ({ toast }))
vi.mock('@/components/container', () => ({
	SceneContainer: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
}))
vi.mock('@/components/GenericEmptyState', () => ({
	default: ({ title }: { title: string }) => <p>{title}</p>,
}))
// the header is shown as its title and the items of its menu, which is what the page decides
vi.mock('@/components/sharedLayout', () => ({
	CollectionSceneHeader: ({
		title,
		actions,
	}: {
		title: string
		actions?: { items: { label: string; onClick: () => void }[] }[]
	}) => (
		<header>
			<h1>{title}</h1>
			<nav aria-label="menu">
				{actions
					?.flatMap((group) => group.items)
					.map((item) => (
						<button key={item.label} onClick={item.onClick}>
							{item.label}
						</button>
					))}
			</nav>
		</header>
	),
}))
vi.mock('../SmartListGridItem', () => ({
	default: ({ data }: { data: { id: string; name: string } }) => (
		<a href={`/${data.id}`}>{data.name}</a>
	),
}))

const SHARE = 'userSmartListsScene.share'

const list = (
	id: string,
	name: string,
	creatorId: string,
	visibility: 'PRIVATE' | 'PUBLIC',
	matchedBooks = 3,
) => ({ id, name, creatorId, visibility, meta: { matchedBooks } })

function renderPage(nodes: ReturnType<typeof list>[], mutation?: () => unknown) {
	const { Wrapper, execute } = setup({
		answers: (query) => {
			if (query.includes('query SmartListsWithSearch')) return { smartLists: nodes }
			if (query.includes('mutation SetSmartListsVisibility')) {
				return mutation ? mutation() : { setSmartListsVisibility: [] }
			}
			throw new Error(`Unexpected query ${query}`)
		},
	})
	render(<UserSmartListsScene />, { wrapper: Wrapper })
	return execute
}

const menu = () => screen.getByRole('navigation', { name: 'menu' })

describe('UserSmartListsScene', () => {
	beforeEach(() => {
		toast.success.mockReset()
		toast.error.mockReset()
	})

	it("lists the user's lists and the public ones of others, in the order the server gives", async () => {
		renderPage([
			list('1', 'Ana recs', 'ana', 'PUBLIC'),
			list('2', 'My shelf', 'me', 'PRIVATE'),
			list('3', 'Zebras', 'ben', 'PUBLIC'),
		])

		await screen.findByText('Ana recs')
		expect(screen.getAllByRole('link').map((link) => link.textContent)).toEqual([
			'Ana recs',
			'My shelf',
			'Zebras',
		])
	})

	it('leaves out a list of someone else that matches none of the books the user may read', async () => {
		renderPage([
			list('1', 'Ana hidden', 'ana', 'PUBLIC', 0),
			list('2', 'Ana recs', 'ana', 'PUBLIC', 2),
		])

		expect(await screen.findByText('Ana recs')).toBeInTheDocument()
		expect(screen.queryByText('Ana hidden')).not.toBeInTheDocument()
	})

	// it's the user's to fill
	it("shows a list of the user's own that matches nothing", async () => {
		renderPage([list('1', 'Nothing yet', 'me', 'PRIVATE', 0)])
		expect(await screen.findByText('Nothing yet')).toBeInTheDocument()
	})

	describe('sharing all the lists', () => {
		it('offers to share only when some of the own lists are private', async () => {
			renderPage([
				list('1', 'Shared one', 'me', 'PUBLIC'),
				list('2', 'Private one', 'me', 'PRIVATE'),
			])
			await screen.findByText('Shared one')

			expect(
				within(menu())
					.getAllByRole('button')
					.map((button) => button.textContent),
			).toEqual([`${SHARE}.all`, `${SHARE}.makePrivate`])
		})

		it('offers nothing when there is nothing to change, or no list of the own', async () => {
			renderPage([
				list('1', 'Ana recs', 'ana', 'PUBLIC'),
				list('2', 'Only private', 'me', 'PRIVATE'),
			])
			await screen.findByText('Ana recs')
			// her list is not the user's to share, so only the user's own private list is offered
			expect(
				within(menu())
					.getAllByRole('button')
					.map((button) => button.textContent),
			).toEqual([`${SHARE}.all`])
		})

		it('has no menu when the user has no list of their own', async () => {
			renderPage([list('1', 'Ana recs', 'ana', 'PUBLIC')])
			await screen.findByText('Ana recs')
			expect(within(menu()).queryAllByRole('button')).toHaveLength(0)
		})

		it('asks first, then shares the private lists of the user and no others', async () => {
			const user = userEvent.setup()
			const execute = renderPage([
				list('1', 'Shared one', 'me', 'PUBLIC'),
				list('2', 'Private one', 'me', 'PRIVATE'),
				list('3', 'Private two', 'me', 'PRIVATE'),
				list('4', 'Ana private?', 'ana', 'PRIVATE'),
			])
			await screen.findByText('Shared one')

			await user.click(within(menu()).getByRole('button', { name: `${SHARE}.all` }))

			// nothing is sent until the user confirms
			expect(await screen.findByText(`${SHARE}.sharing.title`)).toBeInTheDocument()
			expect(sent(execute, 'SetSmartListsVisibility')).toEqual([])

			await user.click(screen.getByRole('button', { name: `${SHARE}.sharing.confirm` }))

			await waitFor(() =>
				expect(sent(execute, 'SetSmartListsVisibility')).toEqual([
					{ ids: ['2', '3'], visibility: 'PUBLIC' },
				]),
			)
			await waitFor(() => expect(toast.success).toHaveBeenCalledWith(`${SHARE}.shared`))
		})

		it('makes the shared lists of the user private after asking', async () => {
			const user = userEvent.setup()
			const execute = renderPage([
				list('1', 'Shared one', 'me', 'PUBLIC'),
				list('2', 'Private one', 'me', 'PRIVATE'),
				list('3', 'Shared by Ana', 'ana', 'PUBLIC'),
			])
			await screen.findByText('Shared one')

			await user.click(within(menu()).getByRole('button', { name: `${SHARE}.makePrivate` }))
			expect(await screen.findByText(`${SHARE}.unsharing.title`)).toBeInTheDocument()
			await user.click(screen.getByRole('button', { name: `${SHARE}.unsharing.confirm` }))

			await waitFor(() =>
				expect(sent(execute, 'SetSmartListsVisibility')).toEqual([
					{ ids: ['1'], visibility: 'PRIVATE' },
				]),
			)
			await waitFor(() => expect(toast.success).toHaveBeenCalledWith(`${SHARE}.madePrivate`))
		})

		it('changes nothing when the user cancels', async () => {
			const user = userEvent.setup()
			const execute = renderPage([list('1', 'Private one', 'me', 'PRIVATE')])
			await screen.findByText('Private one')

			await user.click(within(menu()).getByRole('button', { name: `${SHARE}.all` }))
			await screen.findByText(`${SHARE}.sharing.title`)
			await user.keyboard('{Escape}')

			await waitFor(() =>
				expect(screen.queryByText(`${SHARE}.sharing.title`)).not.toBeInTheDocument(),
			)
			expect(sent(execute, 'SetSmartListsVisibility')).toEqual([])
		})

		it('says so when the server refuses', async () => {
			const user = userEvent.setup()
			renderPage([list('1', 'Private one', 'me', 'PRIVATE')], () => {
				throw new Error('Smart list not found')
			})
			await screen.findByText('Private one')

			await user.click(within(menu()).getByRole('button', { name: `${SHARE}.all` }))
			await user.click(await screen.findByRole('button', { name: `${SHARE}.sharing.confirm` }))

			await waitFor(() =>
				expect(toast.error).toHaveBeenCalledWith(`${SHARE}.error`, {
					description: 'Smart list not found',
				}),
			)
			expect(toast.success).not.toHaveBeenCalled()
		})
	})
})
