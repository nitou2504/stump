import { render, screen } from '@testing-library/react'

import { localeWrapper } from '@/__mocks__/locale'

import URLPageSize from '../URLPageSize'
import URLPagination from '../URLPagination'

vi.mock('../useFilterScene', () => ({
	useURLPageParams: () => ({ pageSize: 20, setPageSize: vi.fn() }),
}))

describe('the page controls', () => {
	it('speak Spanish: the page box, its total, the arrows and the page size', () => {
		render(
			<>
				<URLPagination pages={41} currentPage={2} onChangePage={vi.fn()} />
				<URLPageSize />
			</>,
			{ wrapper: localeWrapper('es-ES') },
		)

		expect(screen.getByRole('spinbutton', { name: 'Página' })).toBeInTheDocument()
		expect(screen.getByText('de 41')).toBeInTheDocument()
		expect(screen.getByRole('button', { name: 'Página anterior' })).toBeInTheDocument()
		expect(screen.getByRole('button', { name: 'Página siguiente' })).toBeInTheDocument()
		expect(screen.getByRole('spinbutton', { name: 'Libros por página' })).toBeInTheDocument()
		expect(screen.getByText('por página')).toBeInTheDocument()
		expect(screen.queryByText('per page')).toBeNull()
		expect(screen.queryByText('of 41')).toBeNull()
	})

	it('read the same in English as before', () => {
		render(
			<>
				<URLPagination pages={41} currentPage={2} onChangePage={vi.fn()} />
				<URLPageSize />
			</>,
			{ wrapper: localeWrapper('en-US') },
		)

		expect(screen.getByRole('spinbutton', { name: 'Page' })).toBeInTheDocument()
		expect(screen.getByText('of 41')).toBeInTheDocument()
		expect(screen.getByRole('button', { name: 'Previous page' })).toBeInTheDocument()
		expect(screen.getByRole('button', { name: 'Next page' })).toBeInTheDocument()
		expect(screen.getByRole('spinbutton', { name: 'Books per page' })).toBeInTheDocument()
		expect(screen.getByText('per page')).toBeInTheDocument()
	})
})
