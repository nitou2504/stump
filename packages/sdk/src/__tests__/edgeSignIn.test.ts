// @vitest-environment jsdom
import { AxiosError, type InternalAxiosRequestConfig } from 'axios'

import { Api, EDGE_SIGN_IN_PAGE } from '../api'

/** An api whose server answers every request with `status` and these headers */
function apiAnswering(status: number, headers: Record<string, string>) {
	const api = new Api({ baseURL: 'http://localhost:10801', authMethod: 'session' })
	api.axios.defaults.adapter = async (config: InternalAxiosRequestConfig) => {
		throw new AxiosError('refused', String(status), config, null, {
			status,
			statusText: '',
			headers,
			config,
			data: '',
		})
	}
	return api
}

describe('the edge asking for a sign-in', () => {
	const assign = vi.fn()
	const original = window.location

	const openAt = (pathname: string) =>
		Object.defineProperty(window, 'location', {
			configurable: true,
			value: { pathname, assign },
		})

	beforeEach(() => {
		assign.mockClear()
		openAt('/series/abc')
	})

	afterAll(() => {
		Object.defineProperty(window, 'location', { configurable: true, value: original })
	})

	it('sends the browser to the sign-in page by address when the edge says so', async () => {
		const api = apiAnswering(404, { 'x-edge-sign-in': '1' })

		await expect(api.axios.get('/anything')).rejects.toBeTruthy()

		expect(assign).toHaveBeenCalledExactlyOnceWith(EDGE_SIGN_IN_PAGE)
		expect(EDGE_SIGN_IN_PAGE).toBe('/api/_i/signin')
	})

	it('does nothing for an error that is not the edge asking', async () => {
		for (const [status, headers] of [
			[404, {}],
			[401, {}],
			[404, { 'x-edge-sign-in': '0' }],
			[500, { 'x-other': '1' }],
		] as const) {
			await expect(apiAnswering(status, { ...headers }).axios.get('/anything')).rejects.toBeTruthy()
		}

		expect(assign).not.toHaveBeenCalled()
	})

	it('does not go round in circles on the server’s own pages', async () => {
		openAt('/api/_i/signin')

		await expect(
			apiAnswering(404, { 'x-edge-sign-in': '1' }).axios.get('/anything'),
		).rejects.toBeTruthy()

		expect(assign).not.toHaveBeenCalled()
	})

	it('does not go round in circles on the web app’s own sign-in and join screens', async () => {
		for (const pathname of ['/auth', '/join']) {
			openAt(pathname)

			await expect(
				apiAnswering(404, { 'x-edge-sign-in': '1' }).axios.get('/anything'),
			).rejects.toBeTruthy()
		}

		expect(assign).not.toHaveBeenCalled()
		// but a page that is not one of them still goes there
		openAt('/authors')
		await expect(
			apiAnswering(404, { 'x-edge-sign-in': '1' }).axios.get('/anything'),
		).rejects.toBeTruthy()
		expect(assign).toHaveBeenCalledOnce()
	})

	it('does not throw where there is a window and no location, as in React Native', async () => {
		Object.defineProperty(window, 'location', { configurable: true, value: undefined })

		await expect(
			apiAnswering(404, { 'x-edge-sign-in': '1' }).axios.get('/anything'),
		).rejects.toMatchObject({ response: { status: 404 } })

		expect(assign).not.toHaveBeenCalled()
	})

	it('still hands the error to the caller', async () => {
		const api = apiAnswering(404, { 'x-edge-sign-in': '1' })

		await expect(api.axios.get('/anything')).rejects.toMatchObject({
			response: { status: 404 },
		})
	})

	it('also holds after the server’s address changes', async () => {
		const api = apiAnswering(404, { 'x-edge-sign-in': '1' })
		api.serviceURL = 'http://localhost:10802'
		api.axios.defaults.adapter = async (config: InternalAxiosRequestConfig) => {
			throw new AxiosError('refused', '404', config, null, {
				status: 404,
				statusText: '',
				headers: { 'x-edge-sign-in': '1' },
				config,
				data: '',
			})
		}

		await expect(api.axios.get('/anything')).rejects.toBeTruthy()

		expect(assign).toHaveBeenCalledOnce()
	})
})
