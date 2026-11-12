import { SDKContext, StumpClientContext } from '@stump/client'
import { Api } from '@stump/sdk'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { PropsWithChildren, Suspense } from 'react'
import { MemoryRouter } from 'react-router'

import { AppContext, IAppContext } from '@/context'

type Answers = (query: string, variables: Record<string, unknown> | undefined) => unknown

/**
 * An app with the server answering through `answers` (the text of the document and its variables
 * to what the server says; throw to refuse), and the owner signed in
 */
export function setup({
	answers,
	path = '/',
	context = {},
}: {
	answers: Answers
	path?: string
	/** What the app knows of the person, over the owner's (a friend is `isServerOwner: false`) */
	context?: Record<string, unknown>
}) {
	const sdk = new Api({ baseURL: 'http://localhost:10801', authMethod: 'session' })
	const client = new QueryClient({
		defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
	})
	const execute = vi.spyOn(sdk, 'execute').mockImplementation(async (document, variables) => {
		return answers(document.toString(), variables as Record<string, unknown> | undefined) as never
	})
	const app = {
		user: { id: 'owner', isServerOwner: true },
		isServerOwner: true,
		checkPermission: () => true,
		...context,
	} as unknown as IAppContext

	function Wrapper({ children }: PropsWithChildren) {
		return (
			<SDKContext.Provider value={{ sdk, setSDK: vi.fn() }}>
				<StumpClientContext.Provider value={{}}>
					<QueryClientProvider client={client}>
						<AppContext.Provider value={app}>
							<MemoryRouter initialEntries={[path]}>
								<Suspense fallback={null}>{children}</Suspense>
							</MemoryRouter>
						</AppContext.Provider>
					</QueryClientProvider>
				</StumpClientContext.Provider>
			</SDKContext.Provider>
		)
	}

	return { execute, client, Wrapper }
}

/** What a mutation or query of that name was sent with, in order */
export const sent = (execute: ReturnType<typeof setup>['execute'], name: string) =>
	execute.mock.calls
		.filter(([document]) => document.toString().match(new RegExp(`(query|mutation) ${name}\\b`)))
		.map(([, variables]) => variables)

/** A page of a list, as the server answers it */
export const page = <T,>(nodes: T[], totalPages = 1) => ({
	nodes,
	pageInfo: { totalPages, totalItems: nodes.length, currentPage: 0 },
})

/** People as the server sends them */
export const person = (id: string, username: string, extra: Record<string, unknown> = {}) => ({
	id,
	username,
	locked: false,
	removed: false,
	isServerOwner: false,
	...extra,
})
