/**
 * Where to go after signing in: the page the person came from (`/auth?redirect=…`), as a path on
 * **this** site, or else the home page.
 *
 * The address is parsed the way the browser will read it (a tab or newline inside it is dropped,
 * a backslash counts as a slash), and only a result on the same origin is used. A pattern that
 * looks at the first characters is not enough: `/\t/evil.com` and ` //evil.com` both pass one and
 * both end up at another site. The sign-in and join screens are where somebody comes from, never
 * where they go, so those go home too.
 */
export function safeRedirect(
	raw: string | null | undefined,
	origin: string = window.location.origin,
): string {
	if (!raw) return '/'

	let url: URL
	try {
		url = new URL(raw, origin)
	} catch {
		return '/'
	}
	if (url.origin !== origin) return '/'

	const path = `${url.pathname}${url.search}${url.hash}`
	return /^\/(?:auth|join)(?:[/?#]|$)/.test(path) ? '/' : path
}
