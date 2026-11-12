import { useGraphQL } from '@stump/client'
import { graphql } from '@stump/graphql'

const internetQuery = graphql(`
	query UserEditorInternet {
		accessPeople {
			publicSignin
			person {
				id
				username
			}
		}
	}
`)

/**
 * The username of the person being edited when they may sign in from the internet (Public
 * access), or nothing. Only the server owner can ask (the list is theirs): for anybody else, and
 * for a new account, it doesn't ask
 */
export function useInternetPerson(
	userId: string | undefined,
	enabled: boolean,
): string | undefined {
	const { data } = useGraphQL(internetQuery, ['userEditor', 'internet'], undefined, {
		enabled: enabled && !!userId,
	})
	return data?.accessPeople.find((row) => row.person.id === userId && row.publicSignin)?.person
		.username
}
