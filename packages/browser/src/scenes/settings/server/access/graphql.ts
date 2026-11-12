import { useGraphQLMutation, useSuspenseGraphQL } from '@stump/client'
import {
	graphql,
	type SignupLinkKind,
	type TypedDocumentString,
	type UserPermission,
} from '@stump/graphql'
import { useQueryClient } from '@tanstack/react-query'
import { useCallback } from 'react'

/** What a query document answers with */
type ResultOf<T> = T extends TypedDocumentString<infer Result, never> ? Result : never

/** The start of the keys of every query of the page: a change on it reloads them all */
export const PUBLIC_ACCESS_KEY = 'publicAccess'

/** How many rows a list of the page holds (the server gives at most 200 a page) */
export const PAGE_SIZE = 50

/** The validities an invite can be given, in hours: an hour, a day, three days, a week, 30 days */
export const INVITE_HOURS = [1, 24, 72, 168, 720] as const
/** How many times an invite can be opened; each opening makes a device */
export const INVITE_USES = [1, 3, 5, 10] as const

const overviewQuery = graphql(`
	query PublicAccessOverview {
		accessOverview {
			hasKey
			hasPublicUrl
			hasStateDir
			configured
			requirePass
			publicUrl
			stateWrittenAt
			activeDevices
			openInvites
			publicSignin
			activeSignupLinks
			signinStats {
				failuresLast10Min
				namesWaiting
				globalBudgetTripped
			}
		}
	}
`)

const peopleQuery = graphql(`
	query PublicAccessPeople {
		accessPeople {
			lastSignInAt
			mustChangePassword
			publicSignin
			maxDevices
			activeDevices
			sessions
			newLibrariesHidden
			signupLinkId
			beyondFriendSet
			hiddenLibraries
			person {
				id
				username
				locked
				removed
				isServerOwner
			}
		}
	}
`)

const invitesQuery = graphql(`
	query PublicAccessInvites($page: Int!, $pageSize: Int!) {
		accessInvites(pagination: { page: $page, pageSize: $pageSize }) {
			nodes {
				id
				label
				createdAt
				createdBy
				expiresAt
				maxUses
				uses
				status
				person {
					id
					username
					removed
				}
			}
			pageInfo {
				totalPages
				totalItems
				currentPage
			}
		}
	}
`)

const devicesQuery = graphql(`
	query PublicAccessDevices($page: Int!, $pageSize: Int!) {
		accessDevices(pagination: { page: $page, pageSize: $pageSize }) {
			nodes {
				id
				label
				legacy
				kind
				createdAt
				firstSeenAt
				lastSeenAt
				lastIp
				lastAgent
				status
				revokedAt
				revokedBy
				person {
					id
					username
					removed
				}
			}
			pageInfo {
				totalPages
				totalItems
				currentPage
			}
		}
	}
`)

const blockedQuery = graphql(`
	query PublicAccessBlocked {
		accessBlockedUsers {
			failuresLast24H
			lastFailureAt
			lockedBy
			person {
				id
				username
			}
		}
	}
`)

const signInsQuery = graphql(`
	query PublicAccessSignIns($failuresOnly: Boolean!, $page: Int!, $pageSize: Int!) {
		accessSignIns(failuresOnly: $failuresOnly, pagination: { page: $page, pageSize: $pageSize }) {
			nodes {
				id
				at
				ip
				userAgent
				successful
				deviceId
				deviceLabel
				reason
				cleared
				person {
					id
					username
					removed
				}
			}
			pageInfo {
				totalPages
				totalItems
				currentPage
			}
		}
	}
`)

const auditQuery = graphql(`
	query PublicAccessAudit($page: Int!, $pageSize: Int!) {
		accessAudit(pagination: { page: $page, pageSize: $pageSize }) {
			nodes {
				id
				at
				actor
				action
				target
				detail
			}
			pageInfo {
				totalPages
				totalItems
				currentPage
			}
		}
	}
`)

const signupLinksQuery = graphql(`
	query PublicAccessSignupLinks($page: Int!, $pageSize: Int!) {
		accessSignupLinks(pagination: { page: $page, pageSize: $pageSize }) {
			nodes {
				id
				label
				kind
				createdAt
				createdBy
				expiresAt
				maxUses
				uses
				status
				revokedAt
				preset {
					permissions
					maxDevices
					ageRestriction {
						age
						restrictOnUnset
					}
					libraries {
						id
						name
					}
				}
				people {
					id
					username
					removed
				}
			}
			pageInfo {
				totalPages
				totalItems
				currentPage
			}
		}
	}
`)

const presetDefaultsQuery = graphql(`
	query PublicAccessPresetDefaults {
		signupPresetDefaults {
			permissions
			maxDevices
			libraries {
				id
				name
			}
		}
	}
`)

const librariesQuery = graphql(`
	query PublicAccessLibraries {
		libraries(pagination: { none: { unpaginated: true } }) {
			nodes {
				id
				name
			}
		}
	}
`)

const warningsQuery = graphql(`
	query PublicAccessWarnings {
		accessWarnings {
			id
			kind
			severity
			count
			since
			detail
			linkId
			person {
				id
				username
				removed
			}
		}
	}
`)

/** What the badge in the sidebar asks: just how many there are (the owner at home only) */
export const warningCountQuery = graphql(`
	query PublicAccessWarningCount {
		accessWarningCount
	}
`)

export type AccessOverview = ResultOf<typeof overviewQuery>['accessOverview']
export type AccessSignupLink = ResultOf<
	typeof signupLinksQuery
>['accessSignupLinks']['nodes'][number]
export type AccessWarning = ResultOf<typeof warningsQuery>['accessWarnings'][number]
export type PresetDefaults = ResultOf<typeof presetDefaultsQuery>['signupPresetDefaults']
export type LibraryChoice = ResultOf<typeof librariesQuery>['libraries']['nodes'][number]
export type AccessPersonRow = ResultOf<typeof peopleQuery>['accessPeople'][number]
export type AccessInvite = ResultOf<typeof invitesQuery>['accessInvites']['nodes'][number]
export type AccessDevice = ResultOf<typeof devicesQuery>['accessDevices']['nodes'][number]
export type AccessBlockedUser = ResultOf<typeof blockedQuery>['accessBlockedUsers'][number]
export type AccessSignIn = ResultOf<typeof signInsQuery>['accessSignIns']['nodes'][number]
export type AccessAuditEntry = ResultOf<typeof auditQuery>['accessAudit']['nodes'][number]
export type PageInfo = ResultOf<typeof invitesQuery>['accessInvites']['pageInfo']

export function useAccessOverview() {
	return useSuspenseGraphQL(overviewQuery, [PUBLIC_ACCESS_KEY, 'overview']).data.accessOverview
}

export function useAccessPeople() {
	return useSuspenseGraphQL(peopleQuery, [PUBLIC_ACCESS_KEY, 'people']).data.accessPeople
}

export function useAccessInvites(page: number) {
	return useSuspenseGraphQL(invitesQuery, [PUBLIC_ACCESS_KEY, 'invites', page], {
		page,
		pageSize: PAGE_SIZE,
	}).data.accessInvites
}

export function useAccessDevices(page: number) {
	return useSuspenseGraphQL(devicesQuery, [PUBLIC_ACCESS_KEY, 'devices', page], {
		page,
		pageSize: PAGE_SIZE,
	}).data.accessDevices
}

export function useAccessBlocked() {
	return useSuspenseGraphQL(blockedQuery, [PUBLIC_ACCESS_KEY, 'blocked']).data.accessBlockedUsers
}

export function useAccessSignIns(failuresOnly: boolean, page: number) {
	return useSuspenseGraphQL(signInsQuery, [PUBLIC_ACCESS_KEY, 'signIns', failuresOnly, page], {
		failuresOnly,
		page,
		pageSize: PAGE_SIZE,
	}).data.accessSignIns
}

export function useAccessAudit(page: number) {
	return useSuspenseGraphQL(auditQuery, [PUBLIC_ACCESS_KEY, 'audit', page], {
		page,
		pageSize: PAGE_SIZE,
	}).data.accessAudit
}

export function useAccessSignupLinks(page: number) {
	return useSuspenseGraphQL(signupLinksQuery, [PUBLIC_ACCESS_KEY, 'signupLinks', page], {
		page,
		pageSize: PAGE_SIZE,
	}).data.accessSignupLinks
}

export function usePresetDefaults() {
	return useSuspenseGraphQL(presetDefaultsQuery, [PUBLIC_ACCESS_KEY, 'presetDefaults']).data
		.signupPresetDefaults
}

/** Every library, for the checklist of what a link's accounts see */
export function useLibraryChoices() {
	return useSuspenseGraphQL(librariesQuery, [PUBLIC_ACCESS_KEY, 'libraries']).data.libraries.nodes
}

export function useAccessWarnings() {
	return useSuspenseGraphQL(warningsQuery, [PUBLIC_ACCESS_KEY, 'warnings']).data.accessWarnings
}

const createInviteMutation = graphql(`
	mutation PublicAccessCreateInvite($input: CreateAccessInviteInput!) {
		createAccessInvite(input: $input) {
			link
			invite {
				id
			}
		}
	}
`)

const revokeInviteMutation = graphql(`
	mutation PublicAccessRevokeInvite($id: ID!) {
		revokeAccessInvite(id: $id)
	}
`)

const revokeDeviceMutation = graphql(`
	mutation PublicAccessRevokeDevice($id: ID!) {
		revokeAccessDevice(id: $id)
	}
`)

const restoreDeviceMutation = graphql(`
	mutation PublicAccessRestoreDevice($id: ID!) {
		restoreAccessDevice(id: $id)
	}
`)

const revokePersonMutation = graphql(`
	mutation PublicAccessRevokePerson($personId: ID!) {
		revokeAccessDevicesOfPerson(personId: $personId)
	}
`)

const resetBudgetMutation = graphql(`
	mutation PublicAccessResetBudget($id: ID!) {
		resetAccessDeviceBudget(id: $id)
	}
`)

const unblockMutation = graphql(`
	mutation PublicAccessUnblock($id: ID!) {
		unblockUser(id: $id)
	}
`)

const setPasswordMutation = graphql(`
	mutation PublicAccessSetPassword($id: ID!, $password: String!, $requireChange: Boolean!) {
		setUserPassword(id: $id, password: $password, requireChange: $requireChange)
	}
`)

const createSignupLinkMutation = graphql(`
	mutation PublicAccessCreateSignupLink($input: CreateSignupLinkInput!) {
		createAccessSignupLink(input: $input) {
			url
			link {
				id
			}
		}
	}
`)

const revokeSignupLinkMutation = graphql(`
	mutation PublicAccessRevokeSignupLink($id: ID!, $disableAccounts: Boolean!) {
		revokeAccessSignupLink(id: $id, disableAccounts: $disableAccounts)
	}
`)

const setPersonAccessMutation = graphql(`
	mutation PublicAccessSetPersonAccess($input: PersonAccessInput!) {
		setPersonAccess(input: $input) {
			publicSignin
			maxDevices
			hiddenLibraries
			newLibrariesHidden
		}
	}
`)

export type CreateInviteInput = {
	personId: string
	label: string
	validForHours: number
	maxUses: number
}

/** What a link gives the accounts it makes */
export type SignupPresetInput = {
	permissions: UserPermission[]
	libraryIds: string[]
	ageRestriction?: { age: number; restrictOnUnset: boolean } | null
	maxDevices: number
}

export type CreateSignupLinkInput = {
	label: string
	kind: SignupLinkKind
	validForHours: number
	maxUses: number
	preset: SignupPresetInput
}

/** What the owner changes about one person's access: what is left out stays as it is */
export type PersonAccessChange = {
	personId: string
	publicSignin?: boolean
	maxDevices?: number
	newLibrariesHidden?: boolean
	onlyLibraryIds?: string[]
}

/**
 * Everything the owner can do on the page. After each change every query of the page is loaded
 * again (an invite made, a device revoked, an account unblocked all show in several places).
 *
 * The link of a new invite is in the response of `createInvite` and nowhere else: Stump keeps a
 * hash of the token. It is returned to the caller and not kept here: the mutation's result is
 * dropped as soon as it settles (`gcTime: 0`), and it is never put in a query's data, so it can't
 * be read from the cache later or end up anywhere that cache is kept.
 */
export function useAccessActions() {
	const client = useQueryClient()
	const refresh = useCallback(
		() => client.invalidateQueries({ queryKey: [PUBLIC_ACCESS_KEY] }),
		[client],
	)
	const options = { onSuccess: refresh }

	const create = useGraphQLMutation(createInviteMutation, { ...options, gcTime: 0 })
	const createSignup = useGraphQLMutation(createSignupLinkMutation, { ...options, gcTime: 0 })
	const revokeSignup = useGraphQLMutation(revokeSignupLinkMutation, options)
	const setAccess = useGraphQLMutation(setPersonAccessMutation, options)
	const revokeInvite = useGraphQLMutation(revokeInviteMutation, options)
	const revokeDevice = useGraphQLMutation(revokeDeviceMutation, options)
	const restoreDevice = useGraphQLMutation(restoreDeviceMutation, options)
	const revokePerson = useGraphQLMutation(revokePersonMutation, options)
	const resetBudget = useGraphQLMutation(resetBudgetMutation, options)
	const unblock = useGraphQLMutation(unblockMutation, options)
	const setPassword = useGraphQLMutation(setPasswordMutation, options)

	return {
		/** Makes an invite and gives back the link to send, which exists only in this answer */
		createInvite: async (input: CreateInviteInput) => {
			const { createAccessInvite } = await create.mutateAsync({ input })
			create.reset()
			return createAccessInvite.link
		},
		/** Makes a link that lets somebody make their own account: the address exists only in this answer */
		createSignupLink: async (input: CreateSignupLinkInput) => {
			const { createAccessSignupLink } = await createSignup.mutateAsync({ input })
			createSignup.reset()
			return createAccessSignupLink.url
		},
		/** Stops a link; with `disableAccounts`, turns off the accounts it made. Says how many */
		revokeSignupLink: async (id: string, disableAccounts: boolean) => {
			const { revokeAccessSignupLink } = await revokeSignup.mutateAsync({ id, disableAccounts })
			return revokeAccessSignupLink
		},
		setPersonAccess: (input: PersonAccessChange) => setAccess.mutateAsync({ input }),
		revokeInvite: (id: string) => revokeInvite.mutateAsync({ id }),
		revokeDevice: (id: string) => revokeDevice.mutateAsync({ id }),
		restoreDevice: (id: string) => restoreDevice.mutateAsync({ id }),
		/** Cuts a person off; says how many devices were revoked */
		revokePerson: async (personId: string) => {
			const { revokeAccessDevicesOfPerson } = await revokePerson.mutateAsync({ personId })
			return revokeAccessDevicesOfPerson
		},
		resetBudget: (id: string) => resetBudget.mutateAsync({ id }),
		unblock: (id: string) => unblock.mutateAsync({ id }),
		setPassword: (id: string, password: string, requireChange: boolean) =>
			setPassword.mutateAsync({ id, password, requireChange }),
		isPending:
			create.isPending ||
			createSignup.isPending ||
			revokeSignup.isPending ||
			setAccess.isPending ||
			revokeInvite.isPending ||
			revokeDevice.isPending ||
			restoreDevice.isPending ||
			revokePerson.isPending ||
			resetBudget.isPending ||
			unblock.isPending ||
			setPassword.isPending,
	}
}
