import { act, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import { ListToolbar, ListToolbarControls } from '../ListToolbar'
import Search from '../Search'
import { openListSearch } from '../searchRegistry'

/** Makes the `(max-width: 639px)` query match, like a phone */
function mockNarrowScreen(isNarrow: boolean) {
	vi.spyOn(window, 'matchMedia').mockImplementation(
		(query: string) =>
			({
				matches: isNarrow && query.includes('max-width'),
				media: query,
				addEventListener: () => {},
				removeEventListener: () => {},
				addListener: () => {},
				removeListener: () => {},
			}) as unknown as MediaQueryList,
	)
}

function renderToolbar({
	hasControls = true,
	initialValue,
}: { hasControls?: boolean; initialValue?: string } = {}) {
	const onChange = vi.fn()
	render(
		<ListToolbar hasControls={hasControls}>
			<Search initialValue={initialValue} onChange={onChange} placeholder="Search books" />
			{hasControls && (
				<ListToolbarControls>
					<button type="button">Filters</button>
				</ListToolbarControls>
			)}
		</ListToolbar>,
	)
	return { onChange, toolbar: document.querySelector('[data-list-toolbar]') as HTMLElement }
}

afterEach(() => vi.restoreAllMocks())

describe('Search', () => {
	it('is a compact, visible input on wider screens', () => {
		mockNarrowScreen(false)
		const { toolbar } = renderToolbar()
		expect(screen.getByRole('searchbox', { name: 'Search books' })).not.toHaveAttribute('tabindex')
		expect(screen.queryByRole('button', { name: 'Search' })).not.toBeInTheDocument()
		expect(toolbar).not.toHaveAttribute('data-search-open')
	})

	it('folds into an icon on a phone and opens across the toolbar', async () => {
		mockNarrowScreen(true)
		const user = userEvent.setup()
		const { toolbar } = renderToolbar()

		const search = document.querySelector('[data-list-toolbar-search]')
		expect(search).toHaveAttribute('data-state', 'collapsed')
		expect(screen.getByRole('searchbox', { hidden: true })).toHaveAttribute('tabindex', '-1')

		await user.click(screen.getByRole('button', { name: 'Search' }))
		expect(search).toHaveAttribute('data-state', 'open')
		expect(screen.getByRole('searchbox')).toHaveFocus()
		// the other controls step aside while it is open
		expect(toolbar).toHaveAttribute('data-search-open', 'true')
		expect(document.querySelector('[data-list-toolbar-controls]')).toHaveClass('max-sm:hidden')

		await user.keyboard('{Escape}')
		expect(search).toHaveAttribute('data-state', 'collapsed')
		expect(toolbar).not.toHaveAttribute('data-search-open')
	})

	it('keeps an active search visible as a dot once folded, and the X clears it', async () => {
		mockNarrowScreen(true)
		const user = userEvent.setup()
		renderToolbar({ initialValue: 'hist' })

		const icon = screen.getByRole('button', { name: 'Search: hist' })
		expect(icon).toHaveAttribute('data-active')

		await user.click(icon)
		expect(screen.getByRole('searchbox')).toHaveValue('hist')
		await user.click(screen.getByRole('button', { name: 'Clear search' }))
		expect(screen.getByRole('searchbox')).toHaveValue('')
		// still open, the X now closes it
		await user.click(screen.getByRole('button', { name: 'Close search' }))
		expect(document.querySelector('[data-list-toolbar-search]')).toHaveAttribute(
			'data-state',
			'collapsed',
		)
	})

	it('keeps the whole row in a toolbar with nothing else (the home page)', () => {
		mockNarrowScreen(true)
		renderToolbar({ hasControls: false })
		expect(document.querySelector('[data-list-toolbar-search]')).toHaveAttribute(
			'data-state',
			'open',
		)
		expect(screen.queryByRole('button', { name: 'Search' })).not.toBeInTheDocument()
	})

	it('opens from a page-wide shortcut through openListSearch', () => {
		mockNarrowScreen(true)
		renderToolbar()
		// jsdom has no layout: say the search is on screen
		vi.spyOn(HTMLElement.prototype, 'offsetParent', 'get').mockReturnValue(document.body)

		let opened = false
		act(() => {
			opened = openListSearch()
		})
		expect(opened).toBe(true)
		expect(screen.getByRole('searchbox')).toHaveFocus()
		expect(document.querySelector('[data-list-toolbar-search]')).toHaveAttribute(
			'data-state',
			'open',
		)
	})

	it('debounces its changes', async () => {
		mockNarrowScreen(false)
		const user = userEvent.setup()
		const { onChange } = renderToolbar()
		await user.type(screen.getByRole('searchbox'), 'eco')
		expect(onChange).not.toHaveBeenCalledWith('eco')
		await act(() => new Promise((resolve) => setTimeout(resolve, 600)))
		expect(onChange).toHaveBeenLastCalledWith('eco')
	})
})
