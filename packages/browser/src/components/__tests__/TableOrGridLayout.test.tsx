import { InterfaceLayout } from '@stump/graphql'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import TableOrGridLayout, { LayoutToggle } from '../TableOrGridLayout'

const setLayoutMode = vi.fn()
let isGrid = true
vi.mock('@/hooks/useLayoutMode', () => ({
	useLayoutMode: () => ({
		isGrid,
		layoutMode: isGrid ? 'GRID' : 'TABLE',
		setLayoutMode,
	}),
}))

afterEach(() => {
	setLayoutMode.mockReset()
	isGrid = true
})

describe('LayoutToggle', () => {
	it('is one button offering the other layout', async () => {
		const user = userEvent.setup()
		const onToggle = vi.fn()
		const { rerender } = render(<LayoutToggle isGrid onToggle={onToggle} />)

		expect(screen.getAllByRole('button')).toHaveLength(1)
		await user.click(screen.getByRole('button', { name: 'Show as table' }))
		expect(onToggle).toHaveBeenCalledTimes(1)

		rerender(<LayoutToggle isGrid={false} onToggle={onToggle} />)
		expect(screen.getByRole('button', { name: 'Show as grid' })).toBeInTheDocument()
	})
})

describe('TableOrGridLayout', () => {
	it('switches the global layout to the other one', async () => {
		const user = userEvent.setup()
		const { unmount } = render(<TableOrGridLayout />)
		await user.click(screen.getByRole('button', { name: 'Show as table' }))
		expect(setLayoutMode).toHaveBeenCalledWith(InterfaceLayout.Table)
		unmount()

		isGrid = false
		render(<TableOrGridLayout />)
		await user.click(screen.getByRole('button', { name: 'Show as grid' }))
		expect(setLayoutMode).toHaveBeenLastCalledWith(InterfaceLayout.Grid)
	})
})
