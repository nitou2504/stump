import '@/__mocks__/resizeObserver'

import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import StatusLine, { hostOf } from '../StatusLine'
import { K, overview, row, serverOf } from './fixtures'
import { setup } from './support'

vi.mock('@/hooks/useTranslate', () => import('./translations'))

function renderLine(data: Parameters<typeof serverOf>[0]) {
	const app = setup({ answers: serverOf(data) })
	render(<StatusLine />, { wrapper: app.Wrapper })
	return app
}

describe('Is it working', () => {
	it('says it in one sentence: where it is open and how many people can sign in', async () => {
		renderLine({
			people: [
				row('ana', 'ana', { publicSignin: true }),
				row('ben', 'ben', { publicSignin: true }),
				row('cy', 'cy'),
			],
		})

		const line = await screen.findByTestId('access-status-line')
		expect(line).toHaveTextContent(`${K('status.open')} host=books.example.org`)
		expect(line).toHaveTextContent(`${K('status.people')} count=2`)
		// none of the technical numbers are on the first view
		expect(screen.queryByText(K('details.failures'))).toBeNull()
	})

	it('does not count the people who cannot sign in', async () => {
		renderLine({ people: [row('ana', 'ana')] })

		expect(await screen.findByTestId('access-status-line')).toHaveTextContent(
			`${K('status.people')} count=0`,
		)
	})

	it('says it is open with device links only while public sign-in is off on the server', async () => {
		renderLine({ overview: overview({ publicSignin: false }) })

		expect(await screen.findByTestId('access-status-line')).toHaveTextContent(
			`${K('status.linksOnly')} host=books.example.org`,
		)
	})

	it('says nobody is let in while the sign-in budget is tripped', async () => {
		renderLine({
			overview: overview({
				signinStats: { failuresLast10Min: 40, namesWaiting: 2, globalBudgetTripped: true },
			}),
		})

		expect(await screen.findByTestId('access-status-line')).toHaveTextContent(K('status.refusing'))
	})

	it('says what is missing, in the owner’s words, instead of the sentence', async () => {
		renderLine({ overview: overview({ configured: false, hasKey: false, hasStateDir: false }) })

		expect(await screen.findByText(K('status.notConfigured.title'))).toBeInTheDocument()
		expect(
			screen.getByText(
				`${K('status.notConfigured.description')} missing=${K('status.missingNames.key')}, ${K('status.missingNames.stateDir')}`,
			),
		).toBeInTheDocument()
		expect(screen.queryByTestId('access-status-line')).toBeNull()
	})

	it('keeps the numbers behind Details', async () => {
		const user = userEvent.setup()
		renderLine({
			overview: overview({
				signinStats: { failuresLast10Min: 7, namesWaiting: 2, globalBudgetTripped: false },
			}),
		})

		await user.click(await screen.findByRole('button', { name: K('status.details') }))

		const details = await screen.findByTestId('access-details')
		expect(within(details).getByText(K('details.title'))).toBeInTheDocument()
		expect(within(details).getByText('https://books.example.org')).toBeInTheDocument()
		// the label and its number sit together
		expect(details.textContent).toContain(`${K('details.failures')}7`)
		expect(details.textContent).toContain(`${K('details.slowed')}2`)
		expect(within(details).getByText(K('details.edgeNote'))).toBeInTheDocument()
	})
})

describe('the host of the address', () => {
	it('is what the owner knows the address by', () => {
		expect(hostOf('https://books.example.org:8443/x')).toBe('books.example.org:8443')
		expect(hostOf('not a url')).toBe('not a url')
		expect(hostOf(null)).toBe('')
	})
})
