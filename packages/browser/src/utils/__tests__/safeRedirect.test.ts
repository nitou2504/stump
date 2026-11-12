import { safeRedirect } from '../safeRedirect'

const ORIGIN = 'https://g3.tail3e401.ts.net:8443'
const go = (raw: string | null | undefined) => safeRedirect(raw, ORIGIN)

describe('safeRedirect', () => {
	it('keeps a path of this site, with its query and hash', () => {
		expect(go('/books/abc')).toBe('/books/abc')
		expect(go('/series/x?page=2#top')).toBe('/series/x?page=2#top')
		expect(go('/api/v2/media/abc/file')).toBe('/api/v2/media/abc/file')
		expect(go(`${ORIGIN}/books/abc`)).toBe('/books/abc')
	})

	it('goes home when there is nothing to go back to', () => {
		for (const raw of [undefined, null, '', '/']) {
			expect(go(raw)).toBe('/')
		}
	})

	it('never leaves the site, however the other address is written', () => {
		for (const raw of [
			'//evil.example',
			'//evil.example/path',
			'/\\evil.example',
			'\\\\evil.example',
			'/\t/evil.example',
			'/\n/evil.example',
			'/\r/evil.example',
			' //evil.example',
			'\t//evil.example',
			'https://evil.example/api',
			'http://evil.example',
			'https://g3.tail3e401.ts.net:8444/books',
			'https://g3.tail3e401.ts.net.evil.example/',
			'javascript:alert(1)',
			'JaVaScRiPt:alert(1)',
			'data:text/html,<script>alert(1)</script>',
			'blob:https://evil.example/x',
			'mailto:a@b.c',
		]) {
			expect(go(raw), JSON.stringify(raw)).toBe('/')
		}
	})

	it('does not send somebody back to the screens they come from', () => {
		for (const raw of [
			'/auth',
			'/auth?redirect=%2Fbooks',
			'/auth/',
			'/auth#x',
			'/join',
			'/join#token',
			'/join/x',
		]) {
			expect(go(raw), raw).toBe('/')
		}
		// but a page that only starts with the same letters is a page
		expect(go('/authors/abc')).toBe('/authors/abc')
		expect(go('/joined')).toBe('/joined')
	})

	it('is not fooled by a path that only looks like a scheme', () => {
		expect(go('/javascript:alert(1)')).toBe('/javascript:alert(1)')
		expect(go('/https://evil.example')).toBe('/https://evil.example')
	})
})
