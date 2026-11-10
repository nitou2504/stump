import { UserPermission } from '@stump/graphql'

import { useAppContext } from '@/context'

/**
 * What a user may do to a book from its page. Every gate on the page is written here and nowhere
 * else, so the row under the cover, its *More* menu and the header can't disagree about who may
 * do what. Hiding is for the user's sake: each of these has a guard on the server as well.
 */
export type BookPermissions = {
	/** The original file: *Download*, and the browser's own PDF viewer, which fetches it */
	canDownload: boolean
	canEmail: boolean
	canEditMetadata: boolean
	canEditThumbnail: boolean
	canAnalyze: boolean
	canRemove: boolean
	/** The manage page has something for this user: its tags, its metadata, a metadata search */
	canManage: boolean
}

export const pickBookPermissions = (
	has: (permission: UserPermission) => boolean,
): BookPermissions => ({
	canDownload: has(UserPermission.DownloadFile),
	canEmail: has(UserPermission.EmailSend) || has(UserPermission.EmailArbitrarySend),
	canEditMetadata: has(UserPermission.EditMetadata),
	canEditThumbnail: has(UserPermission.EditThumbnails),
	canAnalyze: has(UserPermission.ManageLibrary),
	// granted per user by an admin (the user's permissions in Settings → Users)
	canRemove: has(UserPermission.RemoveBook),
	canManage: has(UserPermission.EditMetadata) || has(UserPermission.MetadataFetchRecordManage),
})

export function useBookPermissions(): BookPermissions {
	const { checkPermission } = useAppContext()
	return pickBookPermissions(checkPermission)
}
