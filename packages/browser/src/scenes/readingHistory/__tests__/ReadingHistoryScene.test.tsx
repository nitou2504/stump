import { render, screen } from '@testing-library/react'
import { MemoryRouter, Route, Routes } from 'react-router'

import ReadingHistoryScene from '../index'

const flags = vi.hoisted(() => ({ available: false }))

// read when the scene renders, so a test can turn the page on
vi.mock('../availability', () => ({
	get MY_READING_AVAILABLE() {
		return flags.available
	},
}))
// the page itself is tested where it is: here only where the route leads
vi.mock('../ReadingHistoryPage', () => ({
	default: ({ userId }: { userId?: string }) => <p>{`History page of ${userId ?? 'me'}`}</p>,
}))

function renderAt(path: string) {
	return render(
		<MemoryRouter initialEntries={[path]}>
			<Routes>
				<Route path="/" element={<p>Home page</p>} />
				<Route path="/reading" element={<ReadingHistoryScene />} />
				<Route path="/reading/:userId" element={<ReadingHistoryScene />} />
			</Routes>
		</MemoryRouter>,
	)
}

describe('ReadingHistoryScene', () => {
	describe('while the page is not available', () => {
		beforeEach(() => {
			flags.available = false
		})

		it('sends my reading home', async () => {
			renderAt('/reading')
			expect(await screen.findByText('Home page')).toBeInTheDocument()
		})

		it("sends a friend's reading home too", async () => {
			renderAt('/reading/ana')
			expect(await screen.findByText('Home page')).toBeInTheDocument()
		})
	})

	describe('when the page is available', () => {
		beforeEach(() => {
			flags.available = true
		})

		it("shows the person's own reading at /reading", async () => {
			renderAt('/reading')
			expect(await screen.findByText('History page of me')).toBeInTheDocument()
		})

		it("shows a friend's reading at /reading/:userId", async () => {
			renderAt('/reading/ana')
			expect(await screen.findByText('History page of ana')).toBeInTheDocument()
		})
	})
})
