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

describe('MoreDetails when asked to open', () => {
	const fold = () => screen.getByText('More details').closest('details') as HTMLDetailsElement
	const Folded = ({ open, hint }: { open?: boolean; hint?: string }) => (
		<MoreDetails open={open} hint={hint}>
			<p>Folder</p>
		</MoreDetails>
	)

	it('opens at once, for what has to be fixed in there', () => {
		render(<Folded open />)

		expect(fold().open).toBe(true)
	})

	it('opens later, when it is asked to later', () => {
		const { rerender } = render(<Folded open={false} />)
		expect(fold().open).toBe(false)

		rerender(<Folded open />)

		expect(fold().open).toBe(true)
	})

	it('does not close by itself when what asked for it is gone', () => {
		// the last problem is fixed in here: the fold must not shut under somebody’s hands
		const { rerender } = render(<Folded open />)

		rerender(<Folded open={false} />)

		expect(fold().open).toBe(true)
	})

	it('stays closed once somebody closes it, however often it is drawn again', () => {
		const { rerender } = render(<Folded open />)
		fold().open = false

		rerender(<Folded open hint="folder, language" />)

		expect(fold().open).toBe(false)
	})

	it('is closed when it is never asked to open', () => {
		render(<Folded />)

		expect(fold().open).toBe(false)
	})
})
