import { AccessWarningKind } from '@stump/graphql'

import { describeWarning, nextLimit, sortWarnings } from '../attention'
import type { AccessPersonRow } from '../graphql'
import { row } from './fixtures'

const shown = (kind: AccessWarningKind, extra: Record<string, unknown> = {}) => ({
	kind,
	count: 3,
	detail: null,
	person: { id: 'ana', username: 'ana', removed: false },
	...extra,
})

describe('what a warning says and does', () => {
	it('gives each kind its sentence and its one button', () => {
		expect(describeWarning(shown(AccessWarningKind.AtDeviceLimit))).toEqual({
			key: 'atDeviceLimit',
			action: 'raiseLimit',
		})
		expect(describeWarning(shown(AccessWarningKind.OverDeviceLimit)).action).toBe('seeDevices')
		expect(describeWarning(shown(AccessWarningKind.ManySessions)).action).toBe('signOutAll')
		expect(describeWarning(shown(AccessWarningKind.NotEnabledTried)).action).toBe('letIn')
		expect(describeWarning(shown(AccessWarningKind.NotFriendClass)).action).toBe('fix')
		expect(describeWarning(shown(AccessWarningKind.LinkNearBudget)).action).toBe('newLink')
		expect(describeWarning(shown(AccessWarningKind.AccountLocked)).action).toBe('unblock')
		expect(describeWarning(shown(AccessWarningKind.LeftoverDevices)).action).toBe('removeLeftovers')
		expect(describeWarning(shown(AccessWarningKind.BooksWaiting)).action).toBe('review')
	})

	it('says guesses at names that do not exist in one sentence, with a second part when they were slowed down', () => {
		const names = (detail: string | null, kind = AccessWarningKind.SigninFailures) =>
			describeWarning(shown(kind, { person: null, detail }))
		expect(names('names that do not exist')).toEqual({
			key: 'signinFailuresUnknown',
			action: 'seeAttempts',
		})
		expect(names('slowed').key).toBe('signinFailuresUnknownSlowed')
		// an old server's slow-down of an unknown name reads the same
		expect(names(null, AccessWarningKind.NameBackoff).key).toBe('signinFailuresUnknownSlowed')
		// with a person it is about their password
		expect(describeWarning(shown(AccessWarningKind.SigninFailures)).action).toBe('setPassword')
	})

	it('has no number to say for a lock with no failures on record', () => {
		expect(describeWarning(shown(AccessWarningKind.AccountLocked, { count: 0 })).key).toBe(
			'accountLockedNoCount',
		)
	})
})

describe('the order of the list', () => {
	it('puts the alerts first, then the newest', () => {
		const ordered = sortWarnings([
			{ id: 'old', severity: 'WARN', lastAt: '2026-11-01T00:00:00Z' },
			{ id: 'alert', severity: 'ALERT', lastAt: '2026-10-01T00:00:00Z' },
			{ id: 'new', severity: 'WARN', lastAt: '2026-11-10T00:00:00Z' },
		] as never[])
		expect((ordered as unknown as { id: string }[]).map(({ id }) => id)).toEqual([
			'alert',
			'new',
			'old',
		])
	})
})

describe('raising a limit', () => {
	const people = [
		row('ana', 'ana', { maxDevices: 3 }),
		row('max', 'max', { maxDevices: 10 }),
	] as unknown as AccessPersonRow[]

	it('is one more than they have, and nothing at the most there can be', () => {
		expect(nextLimit(people, 'ana')).toBe(4)
		expect(nextLimit(people, 'max')).toBeNull()
		expect(nextLimit(people, 'nobody')).toBeNull()
	})
})
