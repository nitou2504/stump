import { act, fireEvent, render, screen } from '@testing-library/react'

import Search from '../Search'

describe('Search', () => {
	it('opens with / instead of the browser quick find', () => {
		render(<Search onChange={vi.fn()} placeholder="Search books" />)
		expect(screen.queryByPlaceholderText('Search books')).toBeNull()

		const event = new KeyboardEvent('keydown', { key: '/', bubbles: true, cancelable: true })
		act(() => {
			window.dispatchEvent(event)
		})

		expect(event.defaultPrevented).toBe(true)
		expect(screen.getByPlaceholderText('Search books')).toBeInTheDocument()
	})

	it('leaves / alone while typing in an input', () => {
		render(
			<>
				<input aria-label="other" />
				<Search onChange={vi.fn()} placeholder="Search books" />
			</>,
		)
		const input = screen.getByLabelText('other')
		input.focus()
		const event = new KeyboardEvent('keydown', { key: '/', bubbles: true, cancelable: true })
		input.dispatchEvent(event)

		expect(event.defaultPrevented).toBe(false)
		expect(screen.queryByPlaceholderText('Search books')).toBeNull()
	})

	it('does nothing when disabled', () => {
		render(<Search onChange={vi.fn()} placeholder="Search books" isDisabled />)
		fireEvent.keyDown(window, { key: '/' })
		expect(screen.queryByPlaceholderText('Search books')).toBeNull()
	})
})
