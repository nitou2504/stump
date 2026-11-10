import { render, screen } from '@testing-library/react'

import MoreDetails from '../MoreDetails'

describe('MoreDetails', () => {
	it('is closed until somebody opens it', () => {
		render(
			<MoreDetails>
				<p>Language</p>
			</MoreDetails>,
		)

		const details = screen.getByText('More details').closest('details')
		expect(details).not.toBeNull()
		expect(details).not.toHaveAttribute('open')
	})

	it('holds what it is given', () => {
		render(
			<MoreDetails>
				<p>Language</p>
			</MoreDetails>,
		)

		expect(screen.getByText('More details').closest('details')).toContainElement(
			screen.getByText('Language'),
		)
	})

	it('says what is inside, when it is asked to', () => {
		render(
			<MoreDetails hint="folder, language">
				<p>Language</p>
			</MoreDetails>,
		)

		expect(screen.getByText(/folder, language/)).toBeInTheDocument()
	})

	it('says nothing more than its name without a hint', () => {
		render(
			<MoreDetails>
				<p>Language</p>
			</MoreDetails>,
		)

		expect(screen.getByText('More details').closest('summary')?.textContent).toBe('More details')
	})

	it('opens from its summary, which is what the keyboard and a screen reader press', () => {
		render(
			<MoreDetails>
				<p>Language</p>
			</MoreDetails>,
		)

		// a native disclosure: the first child of the details element is the summary
		const summary = screen.getByText('More details').closest('summary')
		expect(summary).not.toBeNull()
		expect(summary?.parentElement?.tagName).toBe('DETAILS')
		expect(summary?.parentElement?.firstElementChild).toBe(summary)
	})
})
