import { defineConfig, devices } from '@playwright/test'

/**
 * The web app's own sign-in screen on the PUBLIC address, in a real browser, through the real edge
 * (`deploy/public-edge/spa-capture.sh` stands the stack up and runs this). Its own config: the
 * standard e2e run assumes a bare server with no edge and no passes.
 */
export default defineConfig({
	testDir: './tests/public',
	fullyParallel: false,
	workers: 1,
	retries: 0,
	reporter: [['list']],
	timeout: 60_000,
	use: {
		baseURL: process.env.STUMP_PUBLIC_URL ?? 'http://localhost:18381',
		locale: 'en-US',
		trace: 'off',
		video: 'off',
	},
	projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
})
