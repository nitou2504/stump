import { SDKContext, StumpClientContext } from '@stump/client'
import { UserPermission } from '@stump/graphql'
import { Api } from '@stump/sdk'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { PropsWithChildren } from 'react'
import { MemoryRouter } from 'react-router'

import { AppContext, IAppContext } from '@/context'

import { BookActions } from '../BookActions'
import { EDIT_DETAILS_LABEL } from '../bookMoreMenu'

vi.mock('@/hooks/useTranslate', () => ({
	useTranslate: () => ({ t: (key: string) => key, translate: (key: string) => key }),
}))

const book = (extension: string, extra: Record<string, unknown> = {}) => ({
	id: 'book',
	resolvedName: 'Dune',
	pages: 12,
	extension,
	series: { id: 'series' },
	library: { id: 'library' },
	readProgress: null,
	readHistory: [],
	...extra,
})

function setup(permissions: UserPermission[] = []) {
	const sdk = new Api({ baseURL: 'http://localhost:10801', authMethod: 'session' })
	const client = new QueryClient({
		defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
	})
	vi.spyOn(sdk, 'execute').mockImplementation(async (document) => {
		if (document.toString().includes('query BookReadingListIds')) {
			return { mediaById: { id: 'book', readingListIds: [] } }
		}
		throw new Error('Unexpected query')
	})
	const app = {
		user: { id: 'me' },
		checkPermission: (permission: UserPermission) => permissions.includes(permission),
	} as unknown as IAppContext

	function Wrapper({ children }: PropsWithChildren) {
		return (
			<SDKContext.Provider value={{ sdk, setSDK: vi.fn() }}>
				<StumpClientContext.Provider value={{}}>
					<QueryClientProvider client={client}>
						<AppContext.Provider value={app}>
							<MemoryRouter>{children}</MemoryRouter>
						</AppContext.Provider>
					</QueryClientProvider>
				</StumpClientContext.Provider>
			</SDKContext.Provider>
		)
	}

	const onShowInfo = vi.fn()
	const onEditDetails = vi.fn()
	const renderActions = (data: ReturnType<typeof book>) =>
		render(
			// the fragment is read as it is: masking is the type system's
			<BookActions
				fragment={data as never}
				onShowInfo={onShowInfo}
				onEditDetails={onEditDetails}
			/>,
			{ wrapper: Wrapper },
		)

	return { renderActions, onShowInfo, onEditDetails }
}

/** What can be pressed, by name: the links, then the buttons */
const controls = () => ({
	links: screen
		.queryAllByRole('link')
		.map((link) => link.getAttribute('aria-label') || link.textContent),
	buttons: screen.queryAllByRole('button').map((button) => button.getAttribute('aria-label')),
})

const openMore = async (user: ReturnType<typeof userEvent.setup>) => {
	await user.click(await screen.findByRole('button', { name: 'shared.bookActions.more' }))
	return screen.findAllByRole('menuitem')
}

const itemNames = (items: HTMLElement[]) => items.map((item) => item.textContent)

describe('BookActions', () => {
	it('shows a friend Read, Add to list and More, and nothing else', async () => {
		const { renderActions } = setup()
		renderActions(book('cbz'))

		await screen.findByRole('button', { name: 'readingLists.addToList.label' })
		expect(controls()).toEqual({
			links: ['shared.common.read'],
			buttons: ['readingLists.addToList.label', 'shared.bookActions.more'],
		})
	})

	it('adds a Download button for those who may download', async () => {
		const { renderActions } = setup([UserPermission.DownloadFile])
		renderActions(book('cbz'))

		await screen.findByRole('button', { name: 'readingLists.addToList.label' })
		expect(controls().buttons).toEqual([
			'shared.common.download',
			'readingLists.addToList.label',
			'shared.bookActions.more',
		])
	})

	it('says Continue for a book being read and Read Again for one that was finished', async () => {
		const reading = setup()
		const { unmount } = reading.renderActions(book('cbz', { readProgress: { page: 3 } }))
		expect(await screen.findByRole('link', { name: 'shared.common.continue' })).toBeInTheDocument()
		unmount()

		const finished = setup()
		finished.renderActions(book('cbz', { readHistory: [{ __typename: 'ReadingSession' }] }))
		expect(await screen.findByRole('link', { name: 'shared.common.readAgain' })).toBeInTheDocument()
	})

	it("gives a friend's More menu the reading items and the file info", async () => {
		const user = userEvent.setup()
		const { renderActions } = setup()
		renderActions(book('cbz'))

		expect(itemNames(await openMore(user))).toEqual([
			'shared.bookActions.readIncognito',
			'shared.bookActions.markAsRead.label',
			'shared.bookActions.fileInfo',
		])
	})

	it("offers the browser's own PDF viewer only with the permission to download", async () => {
		const user = userEvent.setup()

		const friend = setup()
		const { unmount } = friend.renderActions(book('pdf'))
		expect(itemNames(await openMore(user))).not.toContain('shared.bookActions.openNativePdf')
		unmount()

		const downloader = setup([UserPermission.DownloadFile])
		downloader.renderActions(book('pdf'))
		expect(itemNames(await openMore(user))).toContain('shared.bookActions.openNativePdf')
	})

	it('has nothing in its menu that a user cannot do', async () => {
		const user = userEvent.setup()
		const { renderActions } = setup([UserPermission.EditMetadata])
		renderActions(book('cbz'))

		const items = itemNames(await openMore(user))
		expect(items).toContain(EDIT_DETAILS_LABEL)
		for (const name of [
			'Remove from library',
			'shared.bookActions.analyze',
			'shared.common.email',
		]) {
			expect(items).not.toContain(name)
		}
	})

	it('asks the page to show the file info and to edit the details', async () => {
		const user = userEvent.setup()
		const { renderActions, onShowInfo, onEditDetails } = setup([UserPermission.EditMetadata])
		renderActions(book('cbz'))

		await openMore(user)
		await user.click(screen.getByRole('menuitem', { name: 'shared.bookActions.fileInfo' }))
		expect(onShowInfo).toHaveBeenCalledTimes(1)

		await openMore(user)
		await user.click(screen.getByRole('menuitem', { name: EDIT_DETAILS_LABEL }))
		expect(onEditDetails).toHaveBeenCalledTimes(1)
	})

	it('has no dialog of its own to edit the details: the page has the one', async () => {
		const { renderActions } = setup([UserPermission.EditMetadata])
		const { container } = renderActions(book('cbz'))

		await screen.findByRole('button', { name: 'readingLists.addToList.label' })
		expect(within(container).queryByRole('dialog')).not.toBeInTheDocument()
	})
})
