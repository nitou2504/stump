import { useGraphQLMutation, useSuspenseGraphQL } from '@stump/client'
import { graphql, type TypedDocumentString } from '@stump/graphql'
import { useQueryClient } from '@tanstack/react-query'
import { useCallback } from 'react'

type ResultOf<T> = T extends TypedDocumentString<infer Result, never> ? Result : never

/** The start of the keys of the person's own devices and reading apps: a change reloads them all */
export const MY_ACCESS_KEY = 'myAccess'

const myDevicesQuery = graphql(`
	query MyDevices {
		myDevices {
			active
			limit
			devices {
				id
				label
				kind
				current
				createdAt
				lastSeenAt
			}
		}
	}
`)

const myReadingAppsQuery = graphql(`
	query MyReadingApps {
		myReadingApps {
			id
			label
			url
			createdAt
			lastSeenAt
		}
	}
`)

const renameMutation = graphql(`
	mutation RenameMyDevice($id: ID!, $label: String!) {
		renameMyDevice(id: $id, label: $label) {
			id
			label
		}
	}
`)

const removeMutation = graphql(`
	mutation RemoveMyDevice($id: ID!) {
		removeMyDevice(id: $id)
	}
`)

const createReadingAppMutation = graphql(`
	mutation CreateReadingApp($label: String!) {
		createReadingApp(label: $label) {
			id
			label
			url
			createdAt
			lastSeenAt
		}
	}
`)

const replaceReadingAppMutation = graphql(`
	mutation ReplaceReadingApp($id: ID!) {
		replaceReadingApp(id: $id) {
			id
			label
			url
			createdAt
			lastSeenAt
		}
	}
`)

const removeReadingAppMutation = graphql(`
	mutation RemoveReadingApp($id: ID!) {
		removeReadingApp(id: $id)
	}
`)

export type MyDevices = ResultOf<typeof myDevicesQuery>['myDevices']
export type MyDevice = MyDevices['devices'][number]
export type MyReadingApp = ResultOf<typeof myReadingAppsQuery>['myReadingApps'][number]

export function useMyDevices() {
	return useSuspenseGraphQL(myDevicesQuery, [MY_ACCESS_KEY, 'devices']).data.myDevices
}

export function useMyReadingApps() {
	return useSuspenseGraphQL(myReadingAppsQuery, [MY_ACCESS_KEY, 'readingApps']).data.myReadingApps
}

/**
 * What a person does with their own devices and reading apps. Every change loads both lists again
 * (a reading app is a device too, so the count on one page moves with the other). The address of a
 * reading app is part of its answers, which are kept only as long as the lists are
 */
export function useMyAccessActions() {
	const client = useQueryClient()
	const refresh = useCallback(
		() => client.invalidateQueries({ queryKey: [MY_ACCESS_KEY] }),
		[client],
	)
	const options = { onSuccess: refresh }

	const rename = useGraphQLMutation(renameMutation, options)
	const remove = useGraphQLMutation(removeMutation, options)
	const create = useGraphQLMutation(createReadingAppMutation, options)
	const replace = useGraphQLMutation(replaceReadingAppMutation, options)
	const removeApp = useGraphQLMutation(removeReadingAppMutation, options)

	return {
		renameDevice: (id: string, label: string) => rename.mutateAsync({ id, label }),
		/** Removes a device; the one you are using ends your session too. Says whether it was removed */
		removeDevice: async (id: string) => (await remove.mutateAsync({ id })).removeMyDevice,
		createReadingApp: async (label: string) =>
			(await create.mutateAsync({ label })).createReadingApp,
		replaceReadingApp: async (id: string) => (await replace.mutateAsync({ id })).replaceReadingApp,
		removeReadingApp: async (id: string) => (await removeApp.mutateAsync({ id })).removeReadingApp,
		isPending:
			rename.isPending ||
			remove.isPending ||
			create.isPending ||
			replace.isPending ||
			removeApp.isPending,
	}
}
