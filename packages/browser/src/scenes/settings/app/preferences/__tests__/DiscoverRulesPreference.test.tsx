import '@/__mocks__/resizeObserver'

import { SDKContext, StumpClientContext } from '@stump/client'
import { Api } from '@stump/sdk'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { PropsWithChildren, Suspense } from 'react'

import DiscoverRulesPreference from '../DiscoverRulesPreference'

vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn() } }))

const savedRules = {
	unreadOnly: true,
	excludedTags: ['No picks'],
	excludedGenres: [],
	excludedLibraryIds: [],
	minRating: null,
}

function setup() {
	const sdk = new Api({ baseURL: 'http://localhost:10801', authMethod: 'session' })
	const client = new QueryClient({
		defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
	})
	const execute = vi.spyOn(sdk, 'execute').mockImplementation(async (document, variables) => {
		const query = document.toString()
		if (query.includes('query DiscoverRulesPreference')) return { myDiscoverRules: savedRules }
		if (query.includes('query DiscoverRulesOptions'))
			return {
				tags: [{ id: 1, name: 'No picks' }],
				libraries: { nodes: [{ id: 'g', name: 'Guides', emoji: null }] },
				metadataValueCounts: [
					{ value: 'Level: High School', count: 127 },
					{ value: 'Reference: Dictionaries', count: 12 },
					{ value: 'Fantasy', count: 876 },
					{ value: 'Manga & Comics', count: 154 },
					{ value: 'Light Novels', count: 76 },
				],
			}
		if (query.includes('query DiscoverPoolSize')) {
			const rules = (variables as { rules: typeof savedRules }).rules
			return { discoverPoolSize: 8000 - 100 * rules.excludedGenres.length }
		}
		if (query.includes('mutation UpdateDiscoverRules'))
			return { updateMyDiscoverRules: (variables as { input: typeof savedRules }).input }
		throw new Error('Unexpected query')
	})
	client.setQueryData(['randomMedia', 'week'], { randomMedia: [] })
	function Wrapper({ children }: PropsWithChildren) {
		return (
			<SDKContext.Provider value={{ sdk, setSDK: vi.fn() }}>
				<StumpClientContext.Provider value={{}}>
					<QueryClientProvider client={client}>
						<Suspense fallback={<p>Loading</p>}>{children}</Suspense>
					</QueryClientProvider>
				</StumpClientContext.Provider>
			</SDKContext.Provider>
		)
	}
	return { execute, client, Wrapper }
}

describe('DiscoverRulesPreference', () => {
	it('previews the pool, saves the rules and refreshes the picks', async () => {
		const user = userEvent.setup()
		const { execute, client, Wrapper } = setup()
		render(<DiscoverRulesPreference />, { wrapper: Wrapper })

		expect(await screen.findByText('8,000 books can be picked')).toBeInTheDocument()
		expect(screen.getByText('No picks')).toBeInTheDocument()
		expect(screen.getByRole('button', { name: 'Save' })).toBeDisabled()

		// a common rule, with how many books have it
		await user.click(await screen.findByRole('button', { name: /Level: High School/ }))
		expect(await screen.findByText('7,900 books can be picked')).toBeInTheDocument()
		await user.click(screen.getByRole('switch', { name: 'Only unread books' }))
		await user.click(screen.getByRole('button', { name: 'Save' }))

		await waitFor(() =>
			expect(
				execute.mock.calls.find(([document]) =>
					document.toString().includes('mutation UpdateDiscoverRules'),
				)?.[1],
			).toEqual({
				input: { ...savedRules, unreadOnly: false, excludedGenres: ['Level: High School'] },
			}),
		)
		await waitFor(() =>
			expect(client.getQueryState(['randomMedia', 'week'])?.isInvalidated).toBe(true),
		)
		expect(screen.getByRole('button', { name: 'Save' })).toBeDisabled()
	})

	it('offers families of genres and what was typed', async () => {
		const user = userEvent.setup()
		const { Wrapper } = setup()
		render(<DiscoverRulesPreference />, { wrapper: Wrapper })
		await screen.findByText('8,000 books can be picked')
		await user.click(screen.getByRole('button', { name: 'Add a genre' }))
		expect(await screen.findByRole('option', { name: /Reference: \*/ })).toBeInTheDocument()
		await user.type(screen.getByPlaceholderText('Search genres…'), 'Form: *')
		await user.click(screen.getByRole('option', { name: 'Leave out “Form: *”' }))
		expect(screen.getByRole('button', { name: 'Remove Form: *' })).toBeInTheDocument()
	})

	it('finds genres such as manga from the tag search, and says why the tags are empty', async () => {
		const user = userEvent.setup()
		const { Wrapper } = setup()
		render(<DiscoverRulesPreference />, { wrapper: Wrapper })
		await screen.findByText('8,000 books can be picked')
		await user.click(screen.getByRole('button', { name: 'Add a tag' }))
		// the only tag is already left out
		expect(await screen.findByText(/No other tags yet/)).toBeInTheDocument()
		await user.type(screen.getByPlaceholderText('Search tags…'), 'manga')
		await user.click(await screen.findByRole('option', { name: /Manga & Comics/ }))
		// added as a genre rule, not a tag
		expect(screen.getByRole('button', { name: 'Remove Manga & Comics' })).toBeInTheDocument()
		expect(screen.queryByRole('button', { name: 'Remove manga' })).not.toBeInTheDocument()
		// the common rules offer light novels in one click
		await user.click(screen.getByRole('button', { name: /Light Novels/ }))
		expect(screen.getByRole('button', { name: 'Remove Light Novels' })).toBeInTheDocument()
	})
})
