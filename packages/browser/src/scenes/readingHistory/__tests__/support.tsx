import { SDKContext, StumpClientContext } from '@stump/client'
import { Api } from '@stump/sdk'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { PropsWithChildren } from 'react'
import { MemoryRouter, useLocation } from 'react-router'

type Answers = (query: string, variables: Record<string, unknown> | undefined) => unknown

/** The address the app is at, as text, for a test to read */
export function Location() {
	const { pathname, search } = useLocation()
	return <p data-testid="location">{`${pathname}${search}`}</p>
}

/** An app with the server answering through `answers` (the text of the document and its variables to what the server says; throw to refuse), at `path` */
export function setup({ answers, path = '/reading' }: { answers: Answers; path?: string }) {
	const sdk = new Api({ baseURL: 'http://localhost:10801', authMethod: 'session' })
	const client = new QueryClient({
		defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
	})
	const execute = vi.spyOn(sdk, 'execute').mockImplementation(async (document, variables) => {
		return answers(document.toString(), variables as Record<string, unknown> | undefined) as never
	})

	function Wrapper({ children }: PropsWithChildren) {
		return (
			<SDKContext.Provider value={{ sdk, setSDK: vi.fn() }}>
				<StumpClientContext.Provider value={{}}>
					<QueryClientProvider client={client}>
						<MemoryRouter initialEntries={[path]}>
							{children}
							<Location />
						</MemoryRouter>
					</QueryClientProvider>
				</StumpClientContext.Provider>
			</SDKContext.Provider>
		)
	}

	return { execute, client, Wrapper }
}

/** What a query or mutation of that name was sent with, in order */
export const sent = (execute: ReturnType<typeof setup>['execute'], name: string) =>
	execute.mock.calls
		.filter(([document]) => document.toString().match(new RegExp(`(query|mutation) ${name}\\b`)))
		.map(([, variables]) => variables)

type Book = { id: string; name: string; writers?: string[] }

const media = ({ id, name, writers }: Book) => ({
	id,
	resolvedName: name,
	thumbnail: { url: `/covers/${id}` },
	metadata: writers ? { writers } : null,
})

export const finishedRead = (
	book: Book,
	finishedOn: string,
	readthroughNumber = 1,
	startedOn = finishedOn,
) => ({ media: media(book), readthroughNumber, startedOn, finishedOn })

export const readingNow = (
	book: Book,
	percentage: number | null,
	lastReadOn: string,
	readthroughNumber = 1,
) => ({ media: media(book), percentage, lastReadOn, readthroughNumber })

export const ana = { id: 'ana', username: 'ana', avatarUrl: null }
export const me = { id: 'me', username: 'me', avatarUrl: null }

/** The server's answers for a reading page, with what a test wants to change */
export function history({
	isSelf = true,
	reader = isSelf ? me : ana,
	sharing = false,
	friends = [] as (typeof ana)[],
	reading = [] as ReturnType<typeof readingNow>[],
	years = [] as { year: number; count: number }[],
	finished = [] as ReturnType<typeof finishedRead>[],
	totalPages = 1,
	currentPage = 1,
	refuse = false,
}: {
	isSelf?: boolean
	reader?: typeof me
	sharing?: boolean
	friends?: (typeof ana)[]
	reading?: ReturnType<typeof readingNow>[]
	years?: { year: number; count: number }[]
	finished?: ReturnType<typeof finishedRead>[]
	totalPages?: number
	currentPage?: number
	refuse?: boolean
} = {}): Answers {
	let isSharing = sharing
	return (query, variables) => {
		if (query.includes('query ReadingHistoryPeople'))
			return {
				myReadingPrivacy: { shareReadingHistory: isSharing },
				readersSharingHistory: friends,
			}
		if (query.includes('mutation UpdateMyReadingPrivacy')) {
			isSharing = Boolean(variables?.share)
			return { updateMyReadingPrivacy: { shareReadingHistory: isSharing } }
		}
		// the same refusal for a person who doesn't exist and one who doesn't share
		if (refuse && query.includes('readingHistory(')) throw new Error('Reader not found')
		if (query.includes('query ReadingHistoryReader')) return { readingHistory: { isSelf, reader } }
		if (query.includes('query ReadingHistoryNow')) return { readingHistory: { reading } }
		if (query.includes('query ReadingHistoryFinished'))
			return {
				readingHistory: {
					years,
					finished: {
						nodes: finished,
						pageInfo: { totalItems: finished.length, totalPages, currentPage },
					},
				},
			}
		throw new Error(`Unexpected query ${query}`)
	}
}
