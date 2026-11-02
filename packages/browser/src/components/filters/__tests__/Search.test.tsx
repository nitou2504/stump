import { act, fireEvent, render, screen } from '@testing-library/react'

import { useSlashOpensListSearch } from '../listSearchShortcut'
import Search from '../Search'

function Page({ disabled = false, withInput = false }: { disabled?: boolean; withInput?: boolean }) {
	useSlashOpensListSearch()
	return (
		<>
			{withInput && <input aria-label="other" />}
			<Search onChange={vi.fn()} placeholder="Search books" isDisabled={disabled} />
		</>
	)
}

describe('Search', () => {
	it('opens with / instead of the browser quick find', () => {
		render(<Page />)
		expect(screen.queryByPlaceholderText('Search books')).toBeNull()

		const event = new KeyboardEvent('keydown', { key: '/', bubbles: true, cancelable: true })
		act(() => {
			window.dispatchEvent(event)
		})

		expect(event.defaultPrevented).toBe(true)
		expect(screen.getByPlaceholderText('Search books')).toBeInTheDocument()
	})

	it('leaves / alone while typing in an input', () => {
		render(<Page withInput />)
		const input = screen.getByLabelText('other')
		input.focus()
		const event = new KeyboardEvent('keydown', { key: '/', bubbles: true, cancelable: true })
		input.dispatchEvent(event)

		expect(event.defaultPrevented).toBe(false)
		expect(screen.queryByPlaceholderText('Search books')).toBeNull()
	})

	it('does nothing when disabled', () => {
		render(<Page disabled />)
		fireEvent.keyDown(window, { key: '/' })
		expect(screen.queryByPlaceholderText('Search books')).toBeNull()
	})

	it('leaves / alone on a page without a list search (a reader handles it)', () => {
		function Empty() {
			useSlashOpensListSearch()
			return null
		}
		render(<Empty />)
		const event = new KeyboardEvent('keydown', { key: '/', bubbles: true, cancelable: true })
		window.dispatchEvent(event)
		expect(event.defaultPrevented).toBe(false)
	})
})
