import { AccessWarningKind } from '@stump/graphql'

import type { AccessPersonRow, AccessWarning } from './graphql'

/** What the one button of an item does */
export type AttentionAction =
	| 'raiseLimit'
	| 'seeDevices'
	| 'signOutAll'
	| 'setPassword'
	| 'seeAttempts'
	| 'letIn'
	| 'fix'
	| 'newLink'
	| 'revokeLink'
	| 'unblock'
	| 'removeLeftovers'
	| 'review'

type Description = {
	/** The sentence's key under `attention.kinds`; the unknown-name kinds have a variant */
	key: string
	action: AttentionAction
}

/** The most devices a person may have: the server holds the owner to it, so there is no more to raise to */
export const MOST_DEVICES = 10

/** One line of the list per kind of warning: what it says and what its button does */
const KINDS: Record<AccessWarningKind, Description> = {
	[AccessWarningKind.AtDeviceLimit]: { key: 'atDeviceLimit', action: 'raiseLimit' },
	[AccessWarningKind.OverDeviceLimit]: { key: 'overDeviceLimit', action: 'seeDevices' },
	[AccessWarningKind.ManyDevices]: { key: 'manyDevices', action: 'seeDevices' },
	[AccessWarningKind.DeviceLimitHit]: { key: 'deviceLimitHit', action: 'raiseLimit' },
	[AccessWarningKind.ManySessions]: { key: 'manySessions', action: 'signOutAll' },
	[AccessWarningKind.SigninFailures]: { key: 'signinFailures', action: 'setPassword' },
	[AccessWarningKind.NameBackoff]: { key: 'nameBackoff', action: 'seeAttempts' },
	[AccessWarningKind.GlobalBudget]: { key: 'globalBudget', action: 'seeAttempts' },
	[AccessWarningKind.NotEnabledTried]: { key: 'notEnabledTried', action: 'letIn' },
	[AccessWarningKind.NotFriendClass]: { key: 'notFriendClass', action: 'fix' },
	[AccessWarningKind.LinkNearBudget]: { key: 'linkNearBudget', action: 'newLink' },
	[AccessWarningKind.LinkNoLibraries]: { key: 'linkNoLibraries', action: 'revokeLink' },
	[AccessWarningKind.AccountLocked]: { key: 'accountLocked', action: 'unblock' },
	[AccessWarningKind.LeftoverDevices]: { key: 'leftoverDevices', action: 'removeLeftovers' },
	[AccessWarningKind.BooksWaiting]: { key: 'booksWaiting', action: 'review' },
}

type Shown = Pick<AccessWarning, 'kind' | 'person' | 'count' | 'detail'>

/** The key of the warning's sentence (under `attention.kinds`) and the button it gets */
export function describeWarning(warning: Shown): Description {
	const base = KINDS[warning.kind]
	// failures of names that don't exist (and slow-downs of them) are one sentence, with a second
	// part when Stump slowed them down; they have nobody to set a password for
	if (!warning.person) {
		if (warning.kind === AccessWarningKind.SigninFailures) {
			return {
				key: warning.detail === 'slowed' ? 'signinFailuresUnknownSlowed' : 'signinFailuresUnknown',
				action: 'seeAttempts',
			}
		}
		if (warning.kind === AccessWarningKind.NameBackoff) {
			return { key: 'signinFailuresUnknownSlowed', action: 'seeAttempts' }
		}
	}
	// a lock with no failures on record (locked before this was kept) has no number to say
	if (warning.kind === AccessWarningKind.AccountLocked && warning.count < 1) {
		return { key: 'accountLockedNoCount', action: base.action }
	}
	return base
}

/** The most items the list shows before *Show all* */
export const SHOWN_AT_FIRST = 5

/** Alerts first, then the newest: the order the owner reads the list in */
export function sortWarnings<T extends Pick<AccessWarning, 'severity' | 'lastAt'>>(
	warnings: readonly T[],
): T[] {
	return [...warnings].sort((a, b) => {
		if (a.severity !== b.severity) return a.severity === 'ALERT' ? -1 : 1
		return new Date(b.lastAt).getTime() - new Date(a.lastAt).getTime()
	})
}

/** The limit a person has: what raising it by one starts from; nothing when it can't go higher */
export function nextLimit(
	people: readonly AccessPersonRow[],
	personId: string | undefined,
): number | null {
	const row = people.find(({ person }) => person.id === personId)
	if (!row || row.maxDevices >= MOST_DEVICES) return null
	return row.maxDevices + 1
}
