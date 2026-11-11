import { readdirSync, readFileSync } from 'node:fs'
import { join } from 'node:path'

import en from '../../../../../../../i18n/src/locales/en-US.json'
import es from '../../../../../../../i18n/src/locales/es-ES.json'

const PAGE = join(__dirname, '..')

/** The value at a dotted path of a locale's settings scene, if there is one */
const lookup = (locale: unknown, path: string): unknown =>
	path
		.split('.')
		.reduce<unknown>(
			(node, part) =>
				node && typeof node === 'object' ? (node as Record<string, unknown>)[part] : undefined,
			(locale as { webApp: { settingsScene: Record<string, unknown> } }).webApp.settingsScene[
				'server/access'
			],
		)

/** The arguments of every `accessKey(...)` call of the page's files, with their nesting kept */
function accessKeyCalls(): string[] {
	const calls: string[] = []
	for (const file of readdirSync(PAGE).filter((name) => /\.tsx?$/.test(name))) {
		const source = readFileSync(join(PAGE, file), 'utf8')
		let from = 0
		for (;;) {
			const start = source.indexOf('accessKey(', from)
			if (start === -1) break
			let depth = 1
			let end = start + 'accessKey('.length
			while (end < source.length && depth > 0) {
				if (source[end] === '(') depth += 1
				if (source[end] === ')') depth -= 1
				end += 1
			}
			calls.push(source.slice(start + 'accessKey('.length, end - 1))
			from = end
		}
	}
	return calls
}

/** The keys named as plain strings in the calls (the ones with `${}` are the families below) */
const literalKeys = () =>
	accessKeyCalls().flatMap((call) =>
		// a comparison inside the call (`kind === 'person' ? 'a.b' : 'c.d'`) is not a key
		[...call.replace(/[=!]==?\s*'[^']*'/g, '').matchAll(/'([A-Za-z0-9_.]+)'/g)].map(
			(match) => match[1] ?? '',
		),
	)

/** The keys made by a template string: each family and the values it takes */
const FAMILIES: Record<string, string[]> = {
	'invites.validity.h': ['1', '24', '72', '168', '720'],
	'invites.status.': ['unused', 'used', 'usedUp', 'expired', 'revoked'],
	'signIns.lockedBy.': ['failures', 'owner', 'unknown'],
	'signIns.reason.': ['wrongPassword', 'locked', 'ownerViaEdge', 'wrongPerson'],
	'activity.actions.': [
		'inviteCreate',
		'inviteRevoke',
		'inviteRedeemed',
		'deviceRevoke',
		'deviceRestore',
		'deviceResetBudget',
		'deviceRegisteredLegacy',
		'personRevokeAll',
		'userLock',
		'userLockedByFailures',
		'userUnblock',
		'userPasswordSet',
		'userPasswordChanged',
		'userSessionsDeleted',
		'loginActivityCleared',
	],
}

describe.each([
	['en-US', en],
	['es-ES', es],
])('the words of Public access in %s', (_name, locale) => {
	it('has every key the page asks for', () => {
		const keys = literalKeys()
		expect(keys.length).toBeGreaterThan(80)
		const missing = keys.filter((key) => typeof lookup(locale, key) !== 'string')
		expect(missing).toEqual([])
	})

	it('has every value of the keys the page builds', () => {
		// every family is really built by the page (a renamed one would otherwise be unchecked)
		const calls = accessKeyCalls().join('\n')
		for (const prefix of Object.keys(FAMILIES)) {
			expect(calls, prefix).toContain(`\`${prefix}`)
		}

		const missing = Object.entries(FAMILIES).flatMap(([prefix, values]) =>
			values
				.map((value) => `${prefix}${value}`)
				.filter((key) => typeof lookup(locale, key) !== 'string'),
		)
		expect(missing).toEqual([])
	})

	it('has a label for every action the server writes in the log', () => {
		// the page's own map of actions (the server's names) to keys
		const source = readFileSync(join(PAGE, 'ActivitySection.tsx'), 'utf8')
		const mapped = [...source.matchAll(/'([a-z_.]+)': '([A-Za-z]+)'/g)]
		expect(mapped.length).toBe(FAMILIES['activity.actions.']?.length)
		for (const entry of mapped) {
			expect(typeof lookup(locale, `activity.actions.${entry[2]}`)).toBe('string')
		}
	})

	it('names its sidebar entry, the title of its header, and the screen that asks for a new password', () => {
		const settings = (locale as { webApp: { settingsScene: Record<string, unknown> } }).webApp
			.settingsScene as { sidebar: { management: Record<string, string> } }
		expect(typeof settings.sidebar.management['public access']).toBe('string')
		expect(typeof lookup(locale, 'title')).toBe('string')

		const must = (
			locale as { webApp: { authScene: { mustChangePassword: Record<string, string> } } }
		).webApp.authScene.mustChangePassword
		for (const key of [
			'title',
			'description',
			'newPassword',
			'confirmPassword',
			'mismatch',
			'tooShort',
			'submit',
			'signOut',
			'error',
			'success',
		]) {
			expect(typeof must[key], key).toBe('string')
		}
	})

	it('keeps every value of the page that is filled in by name, in both languages', () => {
		// a placeholder the page doesn't send would show as `{{name}}`: both languages must use the same ones
		const names = (value: unknown) =>
			typeof value === 'string' ? [...value.matchAll(/{{(\w+)}}/g)].map((m) => m[1]).sort() : []
		const walk = (node: unknown, prefix: string): string[] =>
			node && typeof node === 'object'
				? Object.entries(node).flatMap(([key, child]) => walk(child, `${prefix}${key}.`))
				: [prefix.slice(0, -1)]
		const english = (en as unknown as { webApp: { settingsScene: Record<string, unknown> } }).webApp
			.settingsScene['server/access']
		for (const key of walk(english, '')) {
			expect(names(lookup(locale, key)), key).toEqual(names(lookup(en, key)))
		}
	})
})
