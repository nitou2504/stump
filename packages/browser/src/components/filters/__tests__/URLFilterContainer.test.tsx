import { render, screen } from '@testing-library/react'

import URLFilterContainer from '../URLFilterContainer'

vi.mock('../useFilterScene', () => ({
	useURLPageParams: () => ({ pageSize: 20, setPageSize: vi.fn() }),
}))
vi.mock('@/hooks/useTranslate', () => ({
	useTranslate: () => ({ t: (key: string) => key, translate: (key: string) => key }),
}))

describe('URLFilterContainer', () => {
	it('has the page controls in its footer', () => {
		render(
			<URLFilterContainer currentPage={1} pages={3} onChangePage={vi.fn()}>
				<p>the list</p>
			</URLFilterContainer>,
		)
		expect(screen.getByText('the list')).toBeInTheDocument()
		expect(screen.getByTestId('urlFilterFooter')).toBeInTheDocument()
	})

	it('has no footer for a list that fits on one page', () => {
		render(
			<URLFilterContainer currentPage={1} pages={1} onChangePage={vi.fn()} hideFooter>
				<p>the list</p>
			</URLFilterContainer>,
		)
		expect(screen.getByText('the list')).toBeInTheDocument()
		expect(screen.queryByTestId('urlFilterFooter')).toBeNull()
	})
})
