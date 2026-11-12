import { readFileSync } from 'node:fs'
import { join } from 'node:path'

import en from '../../../../../../../i18n/src/locales/en-US.json'
import es from '../../../../../../../i18n/src/locales/es-ES.json'

const lookup = (locale: unknown, scene: string, path: string): unknown =>
	path
		.split('.')
		.reduce<unknown>(
			(node, part) =>
				node && typeof node === 'object' ? (node as Record<string, unknown>)[part] : undefined,
			(locale as { webApp: { settingsScene: Record<string, unknown> } }).webApp.settingsScene[
				scene
			],
		)

/** Every key a scene asks for: the quoted names inside its `key(...)` calls (a choice between two counts as both) */
function keysOf(file: string): string[] {
	const source = readFileSync(join(__dirname, '..', file), 'utf8')
	const found = new Set<string>()
	for (const call of source.matchAll(/\bkey\(([^()]*)\)/g)) {
		for (const quoted of call[1]!.matchAll(/'([^']+)'/g)) found.add(quoted[1]!)
	}
	return [...found]
}

describe.each([
	['MyDevicesScene.tsx', 'app/devices'],
	['ReadingAppsScene.tsx', 'app/readingApps'],
])('%s words', (file, scene) => {
	const keys = keysOf(file)

	it('asks for a good number of them (the scan found its calls)', () => {
		expect(keys.length).toBeGreaterThan(15)
	})

	it.each(['en-US', 'es-ES'])('are all in the %s locale, and none is empty', (name) => {
		const locale = name === 'en-US' ? en : es
		const missing = keys.filter((key) => {
			const value = lookup(locale, scene, key)
			return typeof value !== 'string' || value.trim() === ''
		})
		expect(missing).toEqual([])
	})

	it('are told in Spanish differently from English', () => {
		for (const key of keys) {
			expect(lookup(es, scene, key), key).not.toBe(lookup(en, scene, key))
		}
	})

	it('have the same placeholders in both languages', () => {
		const placeholders = (text: unknown) =>
			[...String(text).matchAll(/{{(\w+)}}/g)].map((m) => m[1]).sort()
		for (const key of keys) {
			expect(placeholders(lookup(es, scene, key)), key).toEqual(
				placeholders(lookup(en, scene, key)),
			)
		}
	})
})

describe('the settings sidebar', () => {
	it.each(['en-US', 'es-ES'])('names the two personal pages in %s', (name) => {
		const sidebar = (
			(name === 'en-US' ? en : es) as {
				webApp: { settingsScene: { sidebar: { personal: Record<string, string> } } }
			}
		).webApp.settingsScene.sidebar.personal
		expect(sidebar['my devices']).toBeTruthy()
		expect(sidebar['reading apps']).toBeTruthy()
	})
})
