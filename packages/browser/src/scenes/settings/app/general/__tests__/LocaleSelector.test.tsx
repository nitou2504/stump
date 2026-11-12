import '@/__mocks__/resizeObserver'

import { render, screen } from '@testing-library/react'

import LocaleSelector from '../LocaleSelector'

vi.mock('@/hooks/useTranslate', () => ({
	useTranslate: () => ({ t: (key: string) => key, translate: (key: string) => key }),
}))

const preferences = vi.hoisted(() => ({ locale: 'es' }))
vi.mock('@/hooks/usePreferences', () => ({
	usePreferences: () => ({ preferences, update: vi.fn() }),
}))

describe('LocaleSelector', () => {
	it('shows the language the server stored as a short code, not the placeholder', () => {
		preferences.locale = 'es'
		render(<LocaleSelector />)
		expect(screen.getByRole('combobox')).toHaveTextContent('Español')
		expect(screen.queryByText('settingsScene.app/account.sections.language.placeholder')).toBeNull()
	})

	it('shows English for an account with no stored language', () => {
		preferences.locale = ''
		render(<LocaleSelector />)
		expect(screen.getByRole('combobox')).toHaveTextContent('English (US)')
	})
})
