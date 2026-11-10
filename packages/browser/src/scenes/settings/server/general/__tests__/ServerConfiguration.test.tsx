import { render, screen } from '@testing-library/react'

import { ServerConfiguration } from '../ServerConfiguration'

vi.mock('@/hooks/useTranslate', () => ({
	useTranslate: () => ({ t: (key: string) => key }),
}))

// the parts are tested where they are; here only which of them the page has
vi.mock('../ServerPublicURL', () => ({
	default: () => <div data-testid="public-url" />,
}))

vi.mock('../ServerEmojisSection', () => ({
	default: () => <div data-testid="custom-emojis" />,
}))

describe('ServerConfiguration', () => {
	it('has the public URL', () => {
		render(<ServerConfiguration />)

		expect(screen.getByTestId('public-url')).toBeInTheDocument()
	})

	it('has no custom emoji section: the emoji were for the book clubs, which are off', () => {
		render(<ServerConfiguration />)

		expect(screen.queryByTestId('custom-emojis')).not.toBeInTheDocument()
	})
})
