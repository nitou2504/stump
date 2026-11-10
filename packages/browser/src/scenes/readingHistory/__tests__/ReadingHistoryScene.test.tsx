import { render, screen } from '@testing-library/react'
import { MemoryRouter, Route, Routes } from 'react-router'

import ReadingHistoryScene from '../index'

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
	// the flag is off in this build: the page is not finished and must not be reachable by a link
	it('sends my reading home while the page is not built', async () => {
		renderAt('/reading')
		expect(await screen.findByText('Home page')).toBeInTheDocument()
	})

	it("sends a friend's reading home too", async () => {
		renderAt('/reading/ana')
		expect(await screen.findByText('Home page')).toBeInTheDocument()
	})
})
