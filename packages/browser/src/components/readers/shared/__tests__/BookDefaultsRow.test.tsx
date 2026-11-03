import { LocaleProvider } from '@stump/i18n'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import { BookDefaultsRow } from '../settingsControls'

function renderRow(differs: boolean) {
	const onMakeDefault = vi.fn()
	const onReset = vi.fn()
	render(
		<LocaleProvider locale="en-US">
			<BookDefaultsRow
				kind="pdf"
				differs={differs}
				onMakeDefault={onMakeDefault}
				onReset={onReset}
			/>
		</LocaleProvider>,
	)
	return { onMakeDefault, onReset }
}

describe('BookDefaultsRow', () => {
	it("offers to make the book's settings every PDF's, or to reset the book", async () => {
		const user = userEvent.setup()
		const { onMakeDefault, onReset } = renderRow(true)
		expect(screen.getByTestId('reader-defaults-state')).toHaveTextContent(
			'This book has its own settings',
		)
		await user.click(screen.getByRole('button', { name: /make default for all PDFs/i }))
		await user.click(screen.getByRole('button', { name: /reset this book/i }))
		expect(onMakeDefault).toHaveBeenCalledTimes(1)
		expect(onReset).toHaveBeenCalledTimes(1)
	})

	it('says the book follows the defaults, with nothing to do', () => {
		renderRow(false)
		expect(screen.getByTestId('reader-defaults-state')).toHaveTextContent(
			/using your PDF defaults/i,
		)
		expect(screen.getByRole('button', { name: /make default for all PDFs/i })).toBeDisabled()
		expect(screen.getByRole('button', { name: /reset this book/i })).toBeDisabled()
	})
})
