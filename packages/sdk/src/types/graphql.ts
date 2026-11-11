import { BookClubMemberRole, User } from '@stump/graphql'

export type GraphQLError = {
	message: string
	locations: Array<{
		line: number
		column: number
	}>
	path?: Array<string | number>
	extensions?: Record<string, unknown>
}

export type GraphQLResponse<T> = {
	data: T
	errors?: Array<GraphQLError>
}

export type AuthUser = Omit<User, 'continueReading'> & {
	/**
	 * The server owner set this user's password and they have to choose their own before they can
	 * use anything else. Only the REST sign-in and viewer routes say so; it is absent otherwise
	 */
	mustChangePassword?: boolean
}

export type BookClubMemberRoleSpec = Record<BookClubMemberRole, string>
