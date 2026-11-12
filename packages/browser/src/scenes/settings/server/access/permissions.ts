import { UserPermission } from '@stump/graphql'

/**
 * What a friend may hold, and no more: the permissions a signup link can give and a person on the
 * internet can have. The server refuses anything else (Upload files and Scan library included), so
 * nothing else is offered
 */
export const FRIEND_PERMISSIONS = [
	UserPermission.ChangePassword,
	UserPermission.ChangeUsername,
	UserPermission.ChangeAvatar,
	UserPermission.DownloadFile,
	UserPermission.AddBooks,
	UserPermission.AccessSmartList,
] as const

/** `SCAN_LIBRARY` as the owner reads it: "Scan library" */
export const permissionWords = (permission: string) =>
	permission
		.toLowerCase()
		.replace(/_/g, ' ')
		.replace(/^./, (letter) => letter.toUpperCase())

/** The permission's name in the locale files: `ADD_BOOKS` is `addBooks` */
export const permissionKey = (permission: string) =>
	permission.toLowerCase().replace(/_([a-z])/g, (_, letter: string) => letter.toUpperCase())

/** The fewest and most devices a person may have (the server holds them to it too) */
export const MIN_DEVICES = 1
export const MAX_DEVICES = 10
