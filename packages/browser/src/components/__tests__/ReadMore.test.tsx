import { LocaleProvider } from '@stump/i18n'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import ReadMore from '../ReadMore'

// The real module boots the whole app; only its debug flag is needed here
vi.mock('../../index.ts', () => ({ DEBUG_ENV: false }))

/** jsdom doesn't lay anything out, so the rendered text's height is set by hand */
function renderWithHeight(height: number) {
	vi.spyOn(HTMLElement.prototype, 'offsetHeight', 'get').mockReturnValue(height)
	return render(
		<LocaleProvider locale="en-US">
			<ReadMore text="A description" />
		</LocaleProvider>,
	)
}

afterEach(() => vi.restoreAllMocks())

describe('ReadMore', () => {
	it('shows a description that fits the preview whole, without a toggle', () => {
		renderWithHeight(200)
		expect(screen.getByText('A description')).toBeInTheDocument()
		expect(screen.queryByRole('button')).not.toBeInTheDocument()
	})

	it("doesn't hide a description only a line or two longer than the preview", () => {
		renderWithHeight(280)
		expect(screen.queryByRole('button')).not.toBeInTheDocument()
	})

	it('collapses a longer description and toggles it in place', async () => {
		const user = userEvent.setup()
		renderWithHeight(800)

		const toggle = screen.getByRole('button', { name: /read more/i })
		const content = document.getElementById(toggle.getAttribute('aria-controls')!)
		expect(content).toHaveStyle({ maxHeight: '240px' })

		await user.click(toggle)
		expect(screen.getByRole('button', { name: /read less/i })).toHaveAttribute(
			'aria-expanded',
			'true',
		)
		expect(content?.style.maxHeight).toBe('')

		await user.click(screen.getByRole('button', { name: /read less/i }))
		expect(content).toHaveStyle({ maxHeight: '240px' })
	})
})
