import { SDKContext, StumpClientContext } from '@stump/client'
import { Api } from '@stump/sdk'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { AxiosError, type InternalAxiosRequestConfig } from 'axios'
import { type PropsWithChildren, type ReactNode, Suspense } from 'react'
import { MemoryRouter, Routes, useLocation } from 'react-router'

export type Answer = { status?: number; data?: unknown }
export type Asked = { key: string; body: unknown; query: string }
type Answers = Record<string, Answer | ((body: unknown) => Answer)>

/**
 * A real SDK whose server is a table: `'POST /auth/login'` → what it answers. The hooks and the
 * screens run for real; what they send is in `requests` (the path is the API's, without `/api/v2`)
 */
export function fakeServer(answers: Answers) {
	const sdk = new Api({ baseURL: 'http://localhost:10801', authMethod: 'session' })
	const requests: Asked[] = []
	sdk.axios.defaults.adapter = async (config: InternalAxiosRequestConfig) => {
		const url = new URL(`${config.baseURL ?? ''}${config.url ?? ''}`)
		const key = `${(config.method ?? 'get').toUpperCase()} ${url.pathname.replace(/^\/api\/v2/, '')}`
		const body =
			typeof config.data === 'string' && config.data ? JSON.parse(config.data) : config.data
		requests.push({ key, body, query: url.search })
		const given = answers[key]
		if (!given) throw new Error(`The fake server has no answer for ${key}`)
		const { status = 200, data = {} } = typeof given === 'function' ? given(body) : given
		const response = { status, statusText: '', headers: {}, config, data }
		if (status >= 400) {
			throw new AxiosError(
				`Request failed with status code ${status}`,
				'ERR_BAD_REQUEST',
				config,
				null,
				response,
			)
		}
		return response
	}
	return { sdk, requests }
}

/** The screens' place in the app: the router, the SDK, a query client and a Suspense boundary */
export function appAround(sdk: Api, entry: string, routes: ReactNode) {
	const client = new QueryClient({
		defaultOptions: { mutations: { retry: false }, queries: { retry: false } },
	})
	return function App({ children }: PropsWithChildren) {
		return (
			<SDKContext.Provider value={{ sdk, setSDK: () => {} }}>
				<StumpClientContext.Provider value={{}}>
					<QueryClientProvider client={client}>
						<MemoryRouter initialEntries={[entry]}>
							<Suspense fallback={null}>
								<Routes>{routes}</Routes>
								{children}
							</Suspense>
						</MemoryRouter>
					</QueryClientProvider>
				</StumpClientContext.Provider>
			</SDKContext.Provider>
		)
	}
}

/** Says where the router is, so a test can see where a screen sent somebody */
export function Where() {
	return <p data-testid="where">{useLocation().pathname}</p>
}

/**
 * Replaces `window.location` with a stand-in whose `assign` can be watched (a page load is not
 * something jsdom does)
 */
export function stubLocation(pathname = '/auth', hash = '') {
	const assign = vi.fn()
	const original = window.location
	Object.defineProperty(window, 'location', {
		configurable: true,
		value: {
			assign,
			hash,
			href: `https://g3.test${pathname}${hash}`,
			origin: 'https://g3.test',
			pathname,
			search: '',
		},
	})
	return {
		assign,
		restore: () =>
			Object.defineProperty(window, 'location', { configurable: true, value: original }),
	}
}

export const USER = {
	id: 'ana-id',
	username: 'ana',
	isServerOwner: false,
	permissions: [],
	mustChangePassword: false,
}
