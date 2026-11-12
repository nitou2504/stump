import { render, screen, waitFor } from '@testing-library/react'

import { type IAppContext, useAppContext } from '@/context'

import AccessWarningsBadge from '../AccessWarningsBadge'

vi.mock('@/context', () => ({ useAppContext: vi.fn() }))
vi.mock('@/hooks/useTranslate', () => ({
	useTranslate: () => ({
		t: (key: string, options?: Record<string, unknown>) =>
			options ? `${key} ${JSON.stringify(options)}` : key,
	}),
}))

const count = vi.hoisted(() => ({ asked: [] as boolean[], answer: 0 }))
vi.mock('@/scenes/settings/server/access/useWarningCount', () => ({
	useAccessWarningCount: (enabled: boolean) => {
		count.asked.push(enabled)
		return enabled ? count.answer : 0
	},
}))

const asOwner = (isServerOwner: boolean) =>
	vi.mocked(useAppContext).mockReturnValue({ isServerOwner } as unknown as IAppContext)

describe('the Public access badge', () => {
	beforeEach(() => {
		count.asked = []
		count.answer = 0
	})

	it('shows how many things there are, for the owner', async () => {
		asOwner(true)
		count.answer = 4

		render(<AccessWarningsBadge />)

		expect(await screen.findByTestId('access-warnings-badge')).toHaveTextContent('4')
		expect(screen.getByText(/access.warnings.badge/)).toHaveClass('sr-only')
	})

	it('shows nothing when there is nothing to look at', async () => {
		asOwner(true)

		render(<AccessWarningsBadge />)

		await waitFor(() => expect(count.asked).toEqual([true]))
		expect(screen.queryByTestId('access-warnings-badge')).toBeNull()
	})

	it('is for the owner alone: nobody else asks, and nobody else sees it', async () => {
		asOwner(false)
		count.answer = 9

		render(<AccessWarningsBadge />)

		await waitFor(() => expect(count.asked).toEqual([false]))
		expect(screen.queryByTestId('access-warnings-badge')).toBeNull()
	})

	it('says 99+ for a hundred or more, and sits inline in a navigation', async () => {
		asOwner(true)
		count.answer = 120

		render(<AccessWarningsBadge variant="inline" />)

		const badge = await screen.findByTestId('access-warnings-badge')
		expect(badge).toHaveTextContent('99+')
		expect(badge).toHaveClass('inline-block')
		expect(badge).not.toHaveClass('absolute')
	})
})
