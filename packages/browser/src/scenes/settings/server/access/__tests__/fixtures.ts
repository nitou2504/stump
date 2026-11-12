/** What the server says, for the tests of Public access */

export const K = (key: string) => `settingsScene.server/access.${key}`

export const person = (id: string, username: string, extra: Record<string, unknown> = {}) => ({
	id,
	username,
	locked: false,
	removed: false,
	isServerOwner: false,
	...extra,
})

/** A row of `accessPeople` */
export const row = (
	id: string,
	username: string,
	rest: Record<string, unknown> = {},
	extra: Record<string, unknown> = {},
) => ({
	person: person(id, username, extra),
	lastSignInAt: null,
	mustChangePassword: false,
	publicSignin: false,
	maxDevices: 3,
	activeDevices: 0,
	sessions: 0,
	newLibrariesHidden: true,
	signupLinkId: null,
	beyondFriendSet: [],
	hiddenLibraries: 0,
	...rest,
})

export const warning = (id: string, kind: string, extra: Record<string, unknown> = {}) => ({
	id,
	kind,
	severity: 'WARN',
	count: 3,
	since: '2026-11-11T10:00:00Z',
	lastAt: '2026-11-11T10:00:00Z',
	detail: null,
	linkId: null,
	person: { id: 'ana', username: 'ana', removed: false },
	...extra,
})

export const overview = (extra: Record<string, unknown> = {}) => ({
	hasKey: true,
	hasPublicUrl: true,
	hasStateDir: true,
	configured: true,
	requirePass: true,
	publicUrl: 'https://books.example.org',
	stateWrittenAt: '2026-11-11T09:00:00Z',
	activeDevices: 3,
	openInvites: 0,
	publicSignin: true,
	activeSignupLinks: 0,
	signinStats: { failuresLast10Min: 0, namesWaiting: 0, globalBudgetTripped: false },
	...extra,
})

export const device = (id: string, extra: Record<string, unknown> = {}) => ({
	id,
	label: id,
	legacy: false,
	kind: 'BROWSER',
	createdAt: '2026-11-01T10:00:00Z',
	firstSeenAt: '2026-11-01T10:00:00Z',
	lastSeenAt: '2026-11-11T10:00:00Z',
	lastIp: '198.51.100.7',
	lastAgent: 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 Chrome/120.0 Safari/537.36',
	status: 'ACTIVE',
	revokedAt: null,
	revokedBy: null,
	person: { id: 'ana', username: 'ana', removed: false },
	...extra,
})

export const signupLink = (id: string, label: string, extra: Record<string, unknown> = {}) => ({
	id,
	label,
	kind: 'SINGLE',
	createdAt: '2026-11-10T10:00:00Z',
	createdBy: 'owner',
	expiresAt: '2026-11-14T10:00:00Z',
	maxUses: 1,
	uses: 0,
	status: 'ACTIVE',
	revokedAt: null,
	preset: {
		permissions: ['CHANGE_PASSWORD', 'ADD_BOOKS'],
		maxDevices: 3,
		ageRestriction: null,
		libraries: [{ id: 'bib', name: 'Biblioteca' }],
	},
	people: [],
	...extra,
})

export const invite = (id: string, label: string, extra: Record<string, unknown> = {}) => ({
	id,
	label,
	createdAt: '2026-11-10T10:00:00Z',
	createdBy: 'owner',
	expiresAt: '2026-11-14T10:00:00Z',
	maxUses: 3,
	uses: 0,
	status: 'UNUSED',
	person: { id: 'ana', username: 'ana', removed: false },
	...extra,
})

export const LIBRARIES = [
	{ id: 'bib', name: 'Biblioteca', excludedUsers: [] },
	{ id: 'manga', name: 'Manga', excludedUsers: [{ id: 'ana' }] },
]

const pageOf = <T>(nodes: T[]) => ({
	nodes,
	pageInfo: { totalPages: 1, totalItems: nodes.length, currentPage: 0 },
})

type Data = Partial<{
	people: unknown[]
	warnings: unknown[]
	overview: ReturnType<typeof overview>
	links: ReturnType<typeof signupLink>[]
	invites: ReturnType<typeof invite>[]
	devices: ReturnType<typeof device>[]
	blocked: unknown[]
	signIns: unknown[]
	audit: unknown[]
	libraries: typeof LIBRARIES
}>

/**
 * The server as the page asks it, for the lists of the page: the open ones when it asks without
 * the ended, every one when it asks with them, one person's devices when it names the person
 */
export function serverOf(data: Data) {
	const links = data.links ?? []
	const invites = data.invites ?? []
	const devices = data.devices ?? []
	return (query: string, variables: Record<string, unknown> | undefined): unknown => {
		if (query.includes('query PublicAccessOverview'))
			return { accessOverview: data.overview ?? overview() }
		if (query.includes('query PublicAccessPeople')) return { accessPeople: data.people ?? [] }
		if (query.includes('query PublicAccessWarnings')) return { accessWarnings: data.warnings ?? [] }
		if (query.includes('query PublicAccessWarningCount'))
			return { accessWarningCount: (data.warnings ?? []).length }
		if (query.includes('query PublicAccessSignupLinks'))
			return {
				accessSignupLinks: pageOf(
					variables?.includeEnded ? links : links.filter(({ status }) => status === 'ACTIVE'),
				),
			}
		if (query.includes('query PublicAccessInvites'))
			return {
				accessInvites: pageOf(
					variables?.includeEnded
						? invites
						: invites.filter(({ status }) => status === 'UNUSED' || status === 'USED'),
				),
			}
		if (query.includes('query PublicAccessDevices')) {
			const mine = variables?.personId
				? devices.filter(({ person }) => person?.id === variables.personId)
				: devices
			return {
				accessDevices: pageOf(
					variables?.includeEnded ? mine : mine.filter(({ status }) => status === 'ACTIVE'),
				),
			}
		}
		if (query.includes('query PublicAccessBlocked'))
			return { accessBlockedUsers: data.blocked ?? [] }
		if (query.includes('query PublicAccessSignIns'))
			return { accessSignIns: pageOf(data.signIns ?? []) }
		if (query.includes('query PublicAccessAudit')) return { accessAudit: pageOf(data.audit ?? []) }
		if (query.includes('query PublicAccessLibraries'))
			return { libraries: { nodes: data.libraries ?? LIBRARIES } }
		if (query.includes('query PublicAccessPresetDefaults'))
			return {
				signupPresetDefaults: {
					permissions: ['CHANGE_PASSWORD', 'CHANGE_AVATAR', 'DOWNLOAD_FILE', 'ADD_BOOKS'],
					maxDevices: 3,
					libraries: [{ id: 'bib', name: 'Biblioteca' }],
				},
			}
		throw new Error(`Unexpected query ${query}`)
	}
}
