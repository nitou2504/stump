/**
 * The web app's own sign-in screen on the public address, in a real browser, through the real
 * edge (see `deploy/public-edge/spa-capture.sh`, which stands the stack up and runs this).
 *
 * What it proves, with no guessing:
 * - a visitor with no pass gets the app's login screen, and every request the page makes before
 *   anybody signs in is one the edge lets through (nothing to another site either), with no
 *   Content-Security-Policy violation;
 * - the screen is the short version (two fields, one line, no mention of invite links);
 * - signing up from an invite link and signing in on another device work, a device gets a pass,
 *   the fourth device is refused in words, the owner can never sign in here;
 * - the link's token never stays in the address bar or in any storage.
 */
import fs from 'node:fs'
import path from 'node:path'

import {
	type APIRequestContext,
	type BrowserContext,
	expect,
	request as apiRequest,
	test,
} from '@playwright/test'

const PUBLIC = process.env.STUMP_PUBLIC_URL ?? 'http://localhost:18381'
const HOME = process.env.STUMP_HOME_URL ?? 'http://127.0.0.1:18380'
const PASS_COOKIE = '__Host-g3-stump'
const OWNER = { username: 'capture-owner', password: 'owner-Harbor-47x' }
const FRIEND = { username: 'capture-ana', password: 'capture-Harbor-47' }

/** The shapes of what a visitor with no pass may ask for (deploy/public-edge/nginx.conf `$edge_open`) */
const OPEN_SHAPES = [
	/^(GET|HEAD) \/(auth|join)?(\?[^#\s]*)?$/,
	/^(GET|HEAD) \/assets\/[A-Za-z0-9][A-Za-z0-9._-]{0,119}\.(js|css|woff2|png|webp|ico)$/,
	/^GET \/api\/v2\/(auth\/viewer|claim|auth\/oidc\/config)$/,
	/^POST \/api\/v2\/auth\/(login|join)$/,
	// fetched by browsers without cookies, and the invite link's own address
	/^GET \/assets\/(manifest\.webmanifest|favicon-\d+x\d+\.png)(\?.*)?$/,
	/^(GET|POST) \/api\/_i\/[A-Za-z0-9_-]*(\?.*)?$/,
]
/** Asked for, refused, and harmless: the service worker registers after the page has loaded */
const KNOWN_REFUSED = ['GET /sw.js']

type Table = { rows: { method: string; path: string; edge: string }[] }
const TABLE: Table = JSON.parse(
	fs.readFileSync(path.resolve(process.cwd(), '../../deploy/public-edge/open-paths.json'), 'utf8'),
)

declare global {
	interface Window {
		__violations: string[]
	}
}

/** Records every request of a context, and every Content-Security-Policy violation of its pages */
async function watch(context: BrowserContext) {
	const asked: string[] = []
	context.on('request', (request) => {
		const url = new URL(request.url())
		const line = `${request.method()} ${url.pathname}${url.search}`
		asked.push(url.origin === new URL(PUBLIC).origin ? line : `ELSEWHERE ${request.url()}`)
	})
	await context.addInitScript(() => {
		window.__violations = []
		document.addEventListener('securitypolicyviolation', (event) => {
			window.__violations.push(`${event.violatedDirective} ${event.blockedURI}`)
		})
	})
	return asked
}

const strangers = (asked: string[]) =>
	asked.filter(
		(line) => !KNOWN_REFUSED.includes(line) && !OPEN_SHAPES.some((shape) => shape.test(line)),
	)

let owner: APIRequestContext
let signupToken = ''
let libraryId = ''

async function ownerGraphql(query: string, variables: Record<string, unknown> = {}) {
	const response = await owner.post('/api/graphql', { data: { query, variables } })
	const body = await response.json()
	expect(body.errors, JSON.stringify(body.errors)).toBeUndefined()
	return body.data
}

async function newSignupLink(): Promise<string> {
	// a link must name at least one library ("otherwise they would see nothing"): the throwaway stack has none, so make an
	// empty one (no scan) the first time
	if (!libraryId) {
		const made = await ownerGraphql(
			`mutation($i: CreateOrUpdateLibraryInput!) { createLibrary(input: $i) { id } }`,
			{ i: { name: 'Biblioteca', path: '/tmp', scanAfterPersist: false } },
		)
		libraryId = String(made.createLibrary.id)
	}
	const data = await ownerGraphql(
		`mutation($i: CreateSignupLinkInput!) { createAccessSignupLink(input: $i) { url } }`,
		{
			i: {
				label: 'capture',
				kind: 'SINGLE',
				validForHours: 24,
				maxUses: 1,
				preset: {
					permissions: ['CHANGE_PASSWORD', 'CHANGE_AVATAR', 'DOWNLOAD_FILE', 'ADD_BOOKS'],
					libraryIds: [libraryId],
					maxDevices: 3,
				},
			},
		},
	)
	return String(data.createAccessSignupLink.url).split('/').pop() as string
}

test.describe.configure({ mode: 'serial' })

test.beforeAll(async () => {
	// the owner, at home (there is no way in from the internet for them): claim the server, sign in
	owner = await apiRequest.newContext({ baseURL: HOME })
	const claimed = await owner.post('/api/v2/auth/register', { data: OWNER })
	expect(claimed.ok(), await claimed.text()).toBeTruthy()
	const login = await owner.post('/api/v2/auth/login', { data: OWNER })
	expect(login.ok(), await login.text()).toBeTruthy()
	signupToken = await newSignupLink()
})

test('a stranger gets the app’s own login screen, the short version, and asks for nothing else', async ({
	browser,
}) => {
	const context = await browser.newContext()
	const asked = await watch(context)
	const page = await context.newPage()

	const document = await page.goto('/')
	// the policy of the shell a visitor with no pass gets, from the app, and never cached
	const policy = (await document?.headerValue('content-security-policy')) ?? ''
	expect(policy).toContain("default-src 'none'")
	expect(policy).toContain("frame-ancestors 'none'")
	expect(policy).not.toContain('unsafe-eval')
	expect(await document?.headerValue('cache-control')).toContain('no-store')

	// the app asks who is signed in (answered by the edge: 401) and goes to its login screen
	await page.waitForURL(/\/auth(\?|$)/)
	await expect(page.locator('input[name="username"]')).toBeVisible()
	await page.waitForLoadState('networkidle')

	expect(await page.locator('input').count()).toBe(2)
	await expect(page.getByText('Forgot your password? Ask the owner.')).toBeVisible()
	expect((await page.locator('body').innerText()).toLowerCase()).not.toContain('invite')
	await expect(page.getByText('Initialize')).toHaveCount(0)

	expect(strangers(asked), 'requests a stranger’s page may not make').toEqual([])
	expect(await page.evaluate(() => window.__violations), 'policy violations').toEqual([])
	// no pass, no session, and nothing set by merely looking
	expect((await context.cookies()).map((cookie) => cookie.name)).toEqual([])

	// the table the edge is checked against has every kind of row the page just used
	const open = TABLE.rows
		.filter((row) => row.edge !== 'closed')
		.map((row) => `${row.method} ${row.path}`)
	expect(open).toContain('GET /api/v2/claim')
	await context.close()
})

test('a wrong password says so, in the form, and sets nothing', async ({ browser }) => {
	const context = await browser.newContext()
	const page = await context.newPage()
	await page.goto('/auth')

	await page.locator('input[name="username"]').fill(FRIEND.username)
	await page.locator('input[name="password"]').fill('not-the-password-1')
	await page.getByTestId('loginOrRegisterButton').click()

	await expect(page.getByRole('alert')).toHaveText('Wrong username or password.')
	expect((await context.cookies()).map((cookie) => cookie.name)).toEqual([])
	await context.close()
})

test('the owner can never sign in here, with the right password either', async ({ browser }) => {
	const context = await browser.newContext()
	const page = await context.newPage()
	await page.goto('/auth')

	await page.locator('input[name="username"]').fill(OWNER.username)
	await page.locator('input[name="password"]').fill(OWNER.password)
	await page.getByTestId('loginOrRegisterButton').click()

	await expect(page.getByRole('alert')).toHaveText('Wrong username or password.')
	expect((await context.cookies()).map((cookie) => cookie.name)).toEqual([])
	await context.close()
})

test('an invite link opens the join screen, the token leaves the address, and the account is made', async ({
	browser,
}) => {
	const context = await browser.newContext()
	const asked = await watch(context)
	const page = await context.newPage()

	await page.goto(`/api/_i/${signupToken}`)
	await page.waitForURL(/\/join$/)
	// the token travelled in the fragment and is gone from the address (and was never sent anywhere)
	expect(page.url()).toBe(`${PUBLIC}/join`)
	await expect(page.locator('input[name="username"]')).toBeVisible()
	expect(await page.locator('input').count()).toBe(3)
	expect(await page.evaluate(() => JSON.stringify([localStorage, sessionStorage]))).not.toContain(
		signupToken,
	)

	await page.locator('input[name="username"]').fill(FRIEND.username)
	await page.locator('input[name="password"]').fill(FRIEND.password)
	await page.locator('input[name="password2"]').fill(FRIEND.password)
	expect(strangers(asked), 'requests before the form was sent').toEqual([])
	expect(await page.evaluate(() => window.__violations)).toEqual([])
	await page.getByTestId('joinButton').click()

	// the app loads again as a signed-in one, and the device has a pass
	await page.waitForURL(`${PUBLIC}/`)
	await expect
		.poll(async () => (await context.cookies()).map((cookie) => cookie.name))
		.toEqual(expect.arrayContaining([PASS_COOKIE, 'stump_session']))
	const viewer = await context.request.get('/api/v2/auth/viewer')
	expect(viewer.status()).toBe(200)
	expect((await viewer.json()).username).toBe(FRIEND.username)

	// a link of one use is spent; opening it again says it does not work
	const again = await browser.newContext()
	const second = await again.newPage()
	await second.goto(`/api/_i/${signupToken}`)
	await second.waitForLoadState('networkidle')
	expect(second.url()).not.toContain('/join#')
	await again.close()
	await context.close()
})

test('on another device a friend just opens the address and signs in, and that device gets a pass', async ({
	browser,
}) => {
	const context = await browser.newContext()
	const page = await context.newPage()
	// a device new to the address fetches the sign-in screen and then the whole app (about 200 files) with nothing cached:
	// the edge must not refuse one of them (the files have a budget of their own; they once used up the API's, and the
	// screen stayed on its loading spinner)
	const refusedFiles: string[] = []
	context.on('response', (response) => {
		const { pathname } = new URL(response.url())
		if (response.status() === 429 && pathname.startsWith('/assets/')) refusedFiles.push(pathname)
	})
	await page.goto('/')
	await page.waitForURL(/\/auth/)

	await page.locator('input[name="username"]').fill(FRIEND.username)
	await page.locator('input[name="password"]').fill(FRIEND.password)
	await page.getByTestId('loginOrRegisterButton').click()

	await page.waitForURL(`${PUBLIC}/`)
	await page.waitForLoadState('networkidle')
	expect(refusedFiles, 'files of the app the edge refused with a 429').toEqual([])
	const names = (await context.cookies()).map((cookie) => cookie.name)
	expect(names).toEqual(expect.arrayContaining([PASS_COOKIE, 'stump_session']))
	expect((await context.request.get('/api/v2/auth/viewer')).status()).toBe(200)

	// the pass alone (no session) is enough to ask the server something; with nothing it is a 404
	const pass = (await context.cookies()).find((cookie) => cookie.name === PASS_COOKIE)
	expect(pass?.secure && pass.httpOnly).toBeTruthy()
	const onlyPass = await apiRequest.newContext({
		baseURL: PUBLIC,
		extraHTTPHeaders: { cookie: `${PASS_COOKIE}=${pass?.value}` },
	})
	expect((await onlyPass.get('/api/v2/ping')).status()).toBe(200)
	const nothing = await apiRequest.newContext({ baseURL: PUBLIC })
	expect((await nothing.get('/api/v2/ping')).status()).toBe(404)
	await onlyPass.dispose()
	await nothing.dispose()
	await context.close()
})

test('the fourth device is refused in words, and the screen stays a form', async ({ browser }) => {
	// the join made the first device and the sign-in above the second: one more makes three
	const third = await browser.newContext()
	const page3 = await third.newPage()
	await page3.goto('/auth')
	await page3.locator('input[name="username"]').fill(FRIEND.username)
	await page3.locator('input[name="password"]').fill(FRIEND.password)
	await page3.getByTestId('loginOrRegisterButton').click()
	await page3.waitForURL(`${PUBLIC}/`)

	const fourth = await browser.newContext()
	const page4 = await fourth.newPage()
	await page4.goto('/auth')
	await page4.locator('input[name="username"]').fill(FRIEND.username)
	await page4.locator('input[name="password"]').fill(FRIEND.password)
	await page4.getByTestId('loginOrRegisterButton').click()

	await expect(page4.getByRole('alert')).toContainText('You already use 3 devices')
	expect((await fourth.cookies()).map((cookie) => cookie.name)).toEqual([])
	await third.close()
	await fourth.close()
})

test('the screen follows the browser’s language', async ({ browser }) => {
	const context = await browser.newContext({ locale: 'es-ES' })
	const page = await context.newPage()
	await page.goto('/auth')

	await expect(page.getByText('¿Olvidaste tu contraseña? Pregunta al administrador.')).toBeVisible()
	await context.close()
})

test('a request for anything else is refused before the app sees it', async () => {
	const stranger = await apiRequest.newContext({ baseURL: PUBLIC })
	// the closed rows a browser would send as they are (it rewrites dot segments and a leading `//`
	// before anything leaves; verify-edge.sh sends those byte for byte)
	const unchanged = (row: { path: string }) =>
		new URL(row.path, 'http://x.invalid').pathname === row.path.split('?')[0]
	const closed = TABLE.rows.filter(
		(row) => row.edge === 'closed' && row.method === 'GET' && unchanged(row),
	)
	expect(closed.length).toBeGreaterThan(20)
	for (const row of closed) {
		const response = await stranger.get(row.path, { maxRedirects: 0, failOnStatusCode: false })
		expect([404, 400], `${row.method} ${row.path}`).toContain(response.status())
	}
	await stranger.dispose()
})
