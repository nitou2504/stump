import { render, screen } from '@testing-library/react'
import { Suspense } from 'react'
import { MemoryRouter, Outlet } from 'react-router'

import { AppRouter } from '../AppRouter'

vi.mock('../AppLayout.tsx', () => ({ AppLayout: () => <Outlet /> }))
vi.mock('../stores', () => ({
	useAppStore: (select: (state: unknown) => unknown) =>
		select({ baseUrl: 'http://localhost:10801' }),
}))
vi.mock('../scenes/home', () => ({ default: () => <p>Home page</p> }))
vi.mock('../scenes/error/FourOhFour.tsx', () => ({ default: () => <p>Not found</p> }))
// book clubs are not part of the product: no route leads to their pages
vi.mock('../scenes/bookClub/BookClubRouter.tsx', () => ({ default: () => <p>Book clubs</p> }))

function renderAt(path: string) {
	return render(
		<MemoryRouter initialEntries={[path]}>
			<Suspense fallback={<p>Loading</p>}>
				<AppRouter />
			</Suspense>
		</MemoryRouter>,
	)
}

describe('AppRouter', () => {
	it('has no book club pages', async () => {
		renderAt('/clubs')

		expect(await screen.findByText('Not found')).toBeInTheDocument()
		expect(screen.queryByText('Book clubs')).not.toBeInTheDocument()
	})

	it('has no book club pages below the root either', async () => {
		renderAt('/clubs/create')

		expect(await screen.findByText('Not found')).toBeInTheDocument()
		expect(screen.queryByText('Book clubs')).not.toBeInTheDocument()
	})

	// the pages are not built yet (their flags are off), so the routes go home
	it("routes my reading, and a friend's, to a page that goes home while it is not built", async () => {
		renderAt('/reading')
		expect(await screen.findByText('Home page')).toBeInTheDocument()
	})

	it("routes a friend's reading too", async () => {
		renderAt('/reading/ana')
		expect(await screen.findByText('Home page')).toBeInTheDocument()
	})
})
