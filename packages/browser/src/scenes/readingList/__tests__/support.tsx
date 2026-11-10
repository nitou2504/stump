import { SDKContext, StumpClientContext } from '@stump/client'
import { Api } from '@stump/sdk'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { PropsWithChildren } from 'react'
import { MemoryRouter } from 'react-router'

import { AppContext, IAppContext } from '@/context'

type Answers = (query: string, variables: Record<string, unknown> | undefined) => unknown

/**
 * An app with the server answering through `answers` (the text of the document and its variables
 * to what the server says; throw to refuse), and `user` signed in at `route`
 */
export function setup({
	answers,
	user = { id: 'me', isServerOwner: false },
	path = '/',
}: {
	answers: Answers
	user?: { id: string; isServerOwner: boolean }
	path?: string
}) {
	const sdk = new Api({ baseURL: 'http://localhost:10801', authMethod: 'session' })
	const client = new QueryClient({
		defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
	})
	const execute = vi.spyOn(sdk, 'execute').mockImplementation(async (document, variables) => {
		return answers(document.toString(), variables as Record<string, unknown> | undefined) as never
	})
	const app = { user } as unknown as IAppContext

	function Wrapper({ children }: PropsWithChildren) {
		return (
			<SDKContext.Provider value={{ sdk, setSDK: vi.fn() }}>
				<StumpClientContext.Provider value={{}}>
					<QueryClientProvider client={client}>
						<AppContext.Provider value={app}>
							<MemoryRouter initialEntries={[path]}>{children}</MemoryRouter>
						</AppContext.Provider>
					</QueryClientProvider>
				</StumpClientContext.Provider>
			</SDKContext.Provider>
		)
	}

	return { execute, Wrapper }
}

/** What a mutation or query of that name was sent with, in order */
export const sent = (execute: ReturnType<typeof setup>['execute'], name: string) =>
	execute.mock.calls
		.filter(([document]) => document.toString().match(new RegExp(`(query|mutation) ${name}\\b`)))
		.map(([, variables]) => variables)
