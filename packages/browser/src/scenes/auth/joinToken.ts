/**
 * The token of the invite link that opened the join screen.
 *
 * The link is `…/api/_i/<token>` and the server sends it on to `/join#<token>`: the token is in the
 * address's **fragment**, which no server ever receives and no `Referer` carries. It is taken out of
 * the address here, and the address is cleaned (`history.replaceState`) **before the app renders
 * anything**: an error screen copies `location.href` into its report, and the browser's history
 * would otherwise keep the token for good.
 *
 * It is kept in memory only (never `localStorage` or `sessionStorage`): a reload of the join screen
 * means opening the link again, which is the safe side.
 */
let token: string | null = null
let captured = false

/** The path the join screen lives at */
export const JOIN_PATH = '/join'

/**
 * Takes the token out of the address bar. Call it before anything renders (the web app's entry
 * point does); it does nothing the second time, and nothing on another page
 */
export function captureJoinToken(): string | null {
	if (captured) return token
	captured = true

	if (typeof window === 'undefined' || window.location.pathname !== JOIN_PATH) return token

	const raw = window.location.hash.replace(/^#/, '')
	if (raw) {
		try {
			token = decodeURIComponent(raw)
		} catch {
			token = raw
		}
		// the address without the fragment, as the same history entry
		window.history.replaceState(
			window.history.state,
			'',
			window.location.pathname + window.location.search,
		)
	}
	return token
}

/** The token the join screen has, if the link that opened it had one */
export function joinToken(): string | null {
	return captureJoinToken()
}

/** Forgets it (after the account is made, and for tests) */
export function forgetJoinToken(): void {
	token = null
}

/** For tests: back to a page that has not been looked at yet */
export function resetJoinTokenForTests(): void {
	token = null
	captured = false
}
