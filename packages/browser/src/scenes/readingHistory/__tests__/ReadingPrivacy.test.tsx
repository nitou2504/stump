import '@/__mocks__/resizeObserver'

import { act, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import { ReadingPrivacyCard, ReadingPrivacyControl } from '../ReadingPrivacy'
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

describe('ReadingPrivacyControl', () => {
	const chipOf = (name: string) => screen.findByRole('button', { name })
	const switchOf = () => screen.findByRole('switch', { name: 'readingHistory.privacy.switch' })

	it('shows nothing until it knows who sees the reading, so it never says private to someone who shares', async () => {
		const answers = history({ sharing: true })
		const { execute, Wrapper } = setup({ answers })
		let release!: () => void
		const gate = new Promise<void>((resolve) => {
			release = resolve
		})
		execute.mockImplementation(async (document, variables) => {
			if (document.toString().includes('query ReadingHistoryPeople')) await gate
			return answers(document.toString(), variables as Record<string, unknown> | undefined) as never
		})
		render(<ReadingPrivacyControl />, { wrapper: Wrapper })

		await waitFor(() => expect(sent(execute, 'ReadingHistoryPeople')).toHaveLength(1))
		expect(screen.queryByRole('button')).not.toBeInTheDocument()

		release()

		// the first thing it says is the true thing
		expect(await chipOf('readingHistory.privacy.sharedLabel')).toBeInTheDocument()
		expect(
			screen.queryByRole('button', { name: 'readingHistory.privacy.privateLabel' }),
		).not.toBeInTheDocument()
	})

	it('shows nothing when it could not be found out', async () => {
		const { execute, Wrapper } = setup({ answers: history() })
		execute.mockRejectedValue(new Error('down'))
		render(<ReadingPrivacyControl />, { wrapper: Wrapper })

		await waitFor(() => expect(execute).toHaveBeenCalled())
		// lets it take in the refusal before looking
		await act(async () => {
			await new Promise((resolve) => setTimeout(resolve, 50))
		})
		expect(screen.queryByRole('button')).not.toBeInTheDocument()
	})

	it('opens and closes from the keyboard, and the focus goes back to the chip', async () => {
		const user = userEvent.setup()
		const { Wrapper } = setup({ answers: history() })
		render(<ReadingPrivacyControl />, { wrapper: Wrapper })

		const chip = await chipOf('readingHistory.privacy.privateLabel')
		chip.focus()
		await user.keyboard('{Enter}')

		const control = await switchOf()
		expect(control).toBeInTheDocument()
		// the panel has a name of its own for a screen reader
		expect(screen.getByRole('dialog', { name: 'readingHistory.privacy.label' })).toBeInTheDocument()
		// focus lands on the switch, and it reads what is shared before it is turned on
		await waitFor(() => expect(control).toHaveFocus())
		expect(control).toHaveAccessibleDescription('readingHistory.privacy.description')

		await user.keyboard('{Escape}')

		await waitFor(() => expect(screen.queryByRole('switch')).not.toBeInTheDocument())
		expect(chip).toHaveFocus()
	})

	it('can be turned on by its words as well as by the switch', async () => {
		const user = userEvent.setup()
		const { execute, Wrapper } = setup({ answers: history() })
		render(<ReadingPrivacyControl />, { wrapper: Wrapper })

		await user.click(await chipOf('readingHistory.privacy.privateLabel'))
		await user.click(await screen.findByText('readingHistory.privacy.switch'))

		await waitFor(() => expect(sent(execute, 'UpdateMyReadingPrivacy')).toEqual([{ share: true }]))
		expect(await chipOf('readingHistory.privacy.sharedLabel')).toBeInTheDocument()
	})

	it('says so when the change could not be saved, and the chip and the switch stay where they were', async () => {
		const user = userEvent.setup()
		const answers = history()
		const { Wrapper } = setup({
			answers: (query, variables) => {
				if (query.includes('mutation UpdateMyReadingPrivacy')) throw new Error('nope')
				return answers(query, variables)
			},
		})
		render(<ReadingPrivacyControl />, { wrapper: Wrapper })

		await user.click(await chipOf('readingHistory.privacy.privateLabel'))
		const control = await switchOf()
		await user.click(control)

		await waitFor(() => expect(control).not.toBeDisabled())
		expect(control).toHaveAttribute('aria-checked', 'false')
		expect(
			screen.getByRole('button', { name: 'readingHistory.privacy.privateLabel' }),
		).toBeVisible()
	})
})
