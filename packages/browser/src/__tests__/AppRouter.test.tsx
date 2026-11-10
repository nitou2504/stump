import { render, screen } from '@testing-library/react'
import { Suspense } from 'react'
import { MemoryRouter, Outlet } from 'react-router'

import { AppRouter } from '../AppRouter'

vi.mock('../AppLayout.tsx', () => ({ AppLayout: () => <Outlet /> }))
vi.mock('../stores', () => ({
	useAppStore: (select: (state: unknown) => unknown) =>
		select({ baseUrl: 'http://localhost:10801' }),
}))
const flags = vi.hoisted(() => ({ myReading: true }))

// read when the page renders, so a test can turn My reading off
vi.mock('../scenes/readingHistory/availability', () => ({
	get MY_READING_AVAILABLE() {
		return flags.myReading
	},
}))
vi.mock('../scenes/readingHistory/ReadingHistoryPage', () => ({
	default: ({ userId }: { userId?: string }) => <p>{`Reading of ${userId ?? 'me'}`}</p>,
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

	it("routes my reading, and a friend's, to the My reading page", async () => {
		flags.myReading = true

		renderAt('/reading')
		expect(await screen.findByText('Reading of me')).toBeInTheDocument()
	})

	it("routes a friend's reading to the same page, with the friend's id", async () => {
		flags.myReading = true

		renderAt('/reading/ana')
		expect(await screen.findByText('Reading of ana')).toBeInTheDocument()
	})

	// a page that is switched off is not reachable by a link
	it('sends the routes home while My reading is switched off', async () => {
		flags.myReading = false

		renderAt('/reading')
		expect(await screen.findByText('Home page')).toBeInTheDocument()
	})

	it("sends a friend's reading home too while it is switched off", async () => {
		flags.myReading = false

		renderAt('/reading/ana')
		expect(await screen.findByText('Home page')).toBeInTheDocument()
	})
})
