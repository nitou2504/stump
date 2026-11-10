import '@/__mocks__/resizeObserver'

import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import { ReadingPrivacyCard } from '../ReadingPrivacy'
import { history, sent, setup } from './support'

vi.mock('@/hooks/useTranslate', () => import('./translations'))

describe('ReadingPrivacyCard', () => {
	it('is the same switch as the page has, in the account settings', async () => {
		const user = userEvent.setup()
		const { execute, Wrapper } = setup({ answers: history() })
		render(<ReadingPrivacyCard />, { wrapper: Wrapper })

		const control = await screen.findByRole('switch', { name: 'readingHistory.privacy.switch' })
		expect(control).toHaveAttribute('aria-checked', 'false')
		expect(screen.getByText('readingHistory.privacy.label')).toBeInTheDocument()

		await user.click(control)

		await waitFor(() => expect(sent(execute, 'UpdateMyReadingPrivacy')).toEqual([{ share: true }]))
		await waitFor(() =>
			expect(screen.getByRole('switch', { name: 'readingHistory.privacy.switch' })).toHaveAttribute(
				'aria-checked',
				'true',
			),
		)
	})

	it('says so when the change could not be saved, and leaves the switch where it was', async () => {
		const user = userEvent.setup()
		const answers = history()
		const { Wrapper } = setup({
			answers: (query, variables) => {
				if (query.includes('mutation UpdateMyReadingPrivacy')) throw new Error('nope')
				return answers(query, variables)
			},
		})
		render(<ReadingPrivacyCard />, { wrapper: Wrapper })

		const control = await screen.findByRole('switch', { name: 'readingHistory.privacy.switch' })
		await user.click(control)

		await waitFor(() => expect(control).not.toBeDisabled())
		expect(control).toHaveAttribute('aria-checked', 'false')
	})
})
