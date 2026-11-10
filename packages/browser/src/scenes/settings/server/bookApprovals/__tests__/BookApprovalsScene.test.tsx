import { render, screen } from '@testing-library/react'
import { MemoryRouter, Route, Routes } from 'react-router'

import BookApprovalsScene from '../index'

describe('BookApprovalsScene', () => {
	// the flag is off in this build: the page is not finished and must not be reachable by a link
	it('sends the book approvals home while the page is not built', async () => {
		render(
			<MemoryRouter initialEntries={['/settings/book-approvals']}>
				<Routes>
					<Route path="/" element={<p>Home page</p>} />
					<Route path="/settings/book-approvals" element={<BookApprovalsScene />} />
				</Routes>
			</MemoryRouter>,
		)
		expect(await screen.findByText('Home page')).toBeInTheDocument()
	})
})
