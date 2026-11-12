import { render, screen } from '@testing-library/react'

import LocalePreferences from '../LocalePreferences'

vi.mock('@/hooks/useTranslate', () => ({
	useTranslate: () => ({ t: (key: string) => key, translate: (key: string) => key }),
}))

const app = vi.hoisted(() => ({ isServerOwner: false }))
vi.mock('@/context', () => ({ useAppContext: () => app }))
vi.mock('../LocaleSelector', () => ({ default: () => <p>language picker</p> }))

const banner = 'settingsScene.app/account.sections.language.contribute'

describe('LocalePreferences', () => {
	it('does not ask a friend to contribute to the translations', () => {
		app.isServerOwner = false
		render(<LocalePreferences />)
		expect(screen.getByText('language picker')).toBeInTheDocument()
		expect(screen.queryByText(banner)).toBeNull()
	})

	it('shows the owner the call for translators', () => {
		app.isServerOwner = true
		render(<LocalePreferences />)
		expect(screen.getByText('language picker')).toBeInTheDocument()
		expect(screen.getAllByText(banner).length).toBeGreaterThan(0)
	})
})
