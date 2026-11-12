import { captureJoinToken, forgetJoinToken, joinToken, resetJoinTokenForTests } from '../joinToken'

const openAt = (url: string) => window.history.replaceState(null, '', url)

describe('the join token', () => {
	beforeEach(() => {
		resetJoinTokenForTests()
		openAt('/')
	})

	it('is taken out of the address at once, and the address is cleaned', () => {
		openAt('/join#abc-DEF_123')

		expect(captureJoinToken()).toBe('abc-DEF_123')

		expect(window.location.pathname).toBe('/join')
		expect(window.location.hash).toBe('')
		expect(window.location.href).not.toContain('abc-DEF_123')
	})

	it('keeps the query and the history entry', () => {
		const state = { key: 'entry' }
		window.history.replaceState(state, '', '/join?x=1#tok')

		captureJoinToken()

		expect(window.location.search).toBe('?x=1')
		expect(window.history.state).toEqual(state)
	})

	it('is kept in memory for the screen, and nowhere else', () => {
		openAt('/join#secret-token')

		captureJoinToken()

		expect(joinToken()).toBe('secret-token')
		expect(localStorage.length).toBe(0)
		expect(sessionStorage.length).toBe(0)
		expect(document.cookie).not.toContain('secret-token')
	})

	it('does nothing the second time, and the screen can ask again', () => {
		openAt('/join#tok')
		expect(captureJoinToken()).toBe('tok')

		openAt('/join')
		expect(captureJoinToken()).toBe('tok')
		expect(joinToken()).toBe('tok')
	})

	it('does nothing on any other page, and leaves its address alone', () => {
		openAt('/auth#not-a-token')

		expect(captureJoinToken()).toBeNull()

		expect(window.location.hash).toBe('#not-a-token')
	})

	it('has no token when the link had none', () => {
		openAt('/join')

		expect(captureJoinToken()).toBeNull()
		expect(joinToken()).toBeNull()
	})

	it('reads an escaped token, and a broken escape as it is', () => {
		openAt('/join#%41bc')
		expect(captureJoinToken()).toBe('Abc')

		resetJoinTokenForTests()
		openAt('/join#%E0%A4%A')
		expect(captureJoinToken()).toBe('%E0%A4%A')
	})

	it('is forgotten once the account is made', () => {
		openAt('/join#tok')
		captureJoinToken()

		forgetJoinToken()

		expect(joinToken()).toBeNull()
	})
})
