import { useGraphQLMutation, useSDK, useSuspenseGraphQL } from '@stump/client'
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
	query PublicAccessInvites($includeEnded: Boolean!, $page: Int!, $pageSize: Int!) {
		accessInvites(includeEnded: $includeEnded, pagination: { page: $page, pageSize: $pageSize }) {
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
	query PublicAccessDevices($personId: ID, $includeEnded: Boolean!, $page: Int!, $pageSize: Int!) {
		accessDevices(
			personId: $personId
			includeEnded: $includeEnded
			pagination: { page: $page, pageSize: $pageSize }
		) {
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
	query PublicAccessSignIns(
		$failuresOnly: Boolean!
		$othersOnly: Boolean!
		$page: Int!
		$pageSize: Int!
	) {
		accessSignIns(
			failuresOnly: $failuresOnly
			othersOnly: $othersOnly
			pagination: { page: $page, pageSize: $pageSize }
		) {
			nodes {
				id
				at
				ip
				userAgent
				agentLabel
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
				targetLabel
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
	query PublicAccessSignupLinks($includeEnded: Boolean!, $page: Int!, $pageSize: Int!) {
		accessSignupLinks(
			includeEnded: $includeEnded
			pagination: { page: $page, pageSize: $pageSize }
		) {
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
				excludedUsers {
					id
				}
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
			lastAt
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

/** The device links: only the open ones, or every one that was made (the ended ones too) */
export function useAccessInvites(includeEnded: boolean, page = 1) {
	return useSuspenseGraphQL(invitesQuery, [PUBLIC_ACCESS_KEY, 'invites', includeEnded, page], {
		includeEnded,
		page,
		pageSize: PAGE_SIZE,
	}).data.accessInvites
}

/**
 * The devices of one person, or of everybody when `personId` is `null`: the ones that work, or
 * every one that ever was (the revoked ones too)
 */
export function useAccessDevices(personId: string | null, includeEnded: boolean, page = 1) {
	return useSuspenseGraphQL(
		devicesQuery,
		[PUBLIC_ACCESS_KEY, 'devices', personId ?? 'all', includeEnded, page],
		{ personId: personId ?? undefined, includeEnded, page, pageSize: PAGE_SIZE },
	).data.accessDevices
}

/** Every device that works, whoever's it is (for what needs cleaning up) */
export function useAllAccessDevices() {
	return useAccessDevices(null, false)
}

/** The accounts that are locked right now, with their failures */
export function useAccessBlocked() {
	return useSuspenseGraphQL(blockedQuery, [PUBLIC_ACCESS_KEY, 'blocked']).data.accessBlockedUsers
}

/** The other people's sign-ins (the owner's own successful ones are left out) */
export function useAccessSignIns(failuresOnly: boolean, page: number) {
	return useSuspenseGraphQL(signInsQuery, [PUBLIC_ACCESS_KEY, 'signIns', failuresOnly, page], {
		failuresOnly,
		othersOnly: true,
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

/** The signup links: only the open ones, or every one that was made (the ended ones too) */
export function useAccessSignupLinks(includeEnded: boolean, page = 1) {
	return useSuspenseGraphQL(
		signupLinksQuery,
		[PUBLIC_ACCESS_KEY, 'signupLinks', includeEnded, page],
		{ includeEnded, page, pageSize: PAGE_SIZE },
	).data.accessSignupLinks
}

export function usePresetDefaults() {
	return useSuspenseGraphQL(presetDefaultsQuery, [PUBLIC_ACCESS_KEY, 'presetDefaults']).data
		.signupPresetDefaults
}

/** Every library and who is kept out of it, for the checklists of what people see */
export function useLibraryChoices() {
	return useSuspenseGraphQL(librariesQuery, [PUBLIC_ACCESS_KEY, 'libraries']).data.libraries.nodes
}

/** The account whose permissions the owner may take some off: what `updateUser` needs to hear back */
export type UserToEdit = ResultOf<typeof userToEditQuery>['userById']

/** Reads the account to edit, once, when the owner asks for it (not part of the page's queries) */
export function useUserLoader() {
	const { sdk } = useSDK()
	return useCallback(
		async (id: string) => (await sdk.execute(userToEditQuery, { id })).userById,
		[sdk],
	)
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

const dismissWarningsMutation = graphql(`
	mutation PublicAccessDismissWarnings($ids: [ID!]) {
		dismissAccessWarnings(ids: $ids)
	}
`)

const restoreWarningsMutation = graphql(`
	mutation PublicAccessRestoreWarnings($ids: [ID!]!) {
		restoreAccessWarnings(ids: $ids)
	}
`)

const userToEditQuery = graphql(`
	query PublicAccessUserToEdit($id: ID!) {
		userById(id: $id) {
			id
			username
			permissions
			maxSessionsAllowed
			ageRestriction {
				age
				restrictOnUnset
			}
		}
	}
`)

const updateUserMutation = graphql(`
	mutation PublicAccessUpdateUser($id: ID!, $input: UpdateUserInput!) {
		updateUser(id: $id, input: $input) {
			id
			permissions
		}
	}
`)

const signOutEverywhereMutation = graphql(`
	mutation PublicAccessSignOutEverywhere($id: ID!) {
		deleteUserSessions(id: $id)
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
	const dismissWarnings = useGraphQLMutation(dismissWarningsMutation, {
		onSuccess: () => {
			// the page and the badge show what is left at once
			void refresh()
			void client.invalidateQueries({ queryKey: [PUBLIC_ACCESS_KEY, 'warningCount'] })
		},
	})
	const restoreWarnings = useGraphQLMutation(restoreWarningsMutation, {
		onSuccess: () => {
			void refresh()
			void client.invalidateQueries({ queryKey: [PUBLIC_ACCESS_KEY, 'warningCount'] })
		},
	})
	const updateUser = useGraphQLMutation(updateUserMutation, options)
	const signOutEverywhere = useGraphQLMutation(signOutEverywhereMutation, options)

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
		/** Marks warnings as seen (all the visible ones without ids); says how many */
		dismissWarnings: async (ids?: string[]) => {
			const { dismissAccessWarnings } = await dismissWarnings.mutateAsync({ ids })
			return dismissAccessWarnings
		},
		/** Shows warnings that were marked as seen again (Undo); says how many */
		restoreWarnings: async (ids: string[]) => {
			const { restoreAccessWarnings } = await restoreWarnings.mutateAsync({ ids })
			return restoreAccessWarnings
		},
		/** Takes `remove` off an account's permissions, keeping the rest of the account as it is */
		removePermissions: async (user: UserToEdit, remove: readonly UserPermission[]) => {
			await updateUser.mutateAsync({
				id: user.id,
				input: {
					username: user.username,
					permissions: user.permissions.filter((permission) => !remove.includes(permission)),
					maxSessionsAllowed: user.maxSessionsAllowed,
					ageRestriction: user.ageRestriction
						? {
								age: user.ageRestriction.age,
								restrictOnUnset: user.ageRestriction.restrictOnUnset,
							}
						: null,
				},
			})
		},
		/** Ends every session of the account; says how many */
		signOutEverywhere: async (id: string) => {
			const { deleteUserSessions } = await signOutEverywhere.mutateAsync({ id })
			return deleteUserSessions
		},
		isPending:
			updateUser.isPending ||
			signOutEverywhere.isPending ||
			dismissWarnings.isPending ||
			restoreWarnings.isPending ||
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
