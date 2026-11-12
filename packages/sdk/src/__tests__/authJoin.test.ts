import { type InternalAxiosRequestConfig } from 'axios'

import { Api } from '../api'

/** An api whose server answers a request with `data`, and remembers what it was asked */
function apiAnswering(data: unknown) {
	const api = new Api({ baseURL: 'http://localhost:10801', authMethod: 'session' })
	const seen: InternalAxiosRequestConfig[] = []
	api.axios.defaults.adapter = async (config: InternalAxiosRequestConfig) => {
		seen.push(config)
		return { data, status: 200, statusText: '', headers: {}, config }
	}
	return { api, seen }
}

describe('joining with an invite link', () => {
	it('posts the token, the name and the password to the join route, and nothing else', async () => {
		const { api, seen } = apiAnswering({ id: 'u1', username: 'ana' })

		const user = await api.auth.join({
			token: 'TOK_en-1',
			username: 'ana',
			password: 'tulip-Harbor-47',
		})

		expect(user).toEqual({ id: 'u1', username: 'ana' })
		expect(seen).toHaveLength(1)
		const [request] = seen
		expect(request?.method).toBe('post')
		expect(request?.baseURL).toBe('http://localhost:10801/api/v2')
		expect(request?.url).toBe('/auth/join')
		expect(JSON.parse(String(request?.data))).toEqual({
			password: 'tulip-Harbor-47',
			token: 'TOK_en-1',
			username: 'ana',
		})
		// the link's token is in the body, never in the address
		expect(String(request?.url)).not.toContain('TOK_en-1')
	})

	it('has its own query key', () => {
		expect(new Api({ baseURL: 'http://localhost:10801' }).auth.keys.join).toBe('auth.join')
	})
})

describe('the claim check', () => {
	it('tells a public address from a private one', async () => {
		const { api } = apiAnswering({ isClaimed: true, isPublic: true })

		const { data } = await api.server.claimedStatus()

		expect(data.isPublic).toBe(true)
		expect(data.isClaimed).toBe(true)
	})
})
