import { graphql } from '@stump/graphql'

/** The books in Deleted books, in every library the user can see (or one) */
export const deletedBooksQuery = graphql(`
	query DeletedBooks($libraryId: ID) {
		deletedBooks(libraryId: $libraryId) {
			deletedAt
			deletedBy
			expiresAt
			purgeAt
			libraryId
			libraryName
			relativePath
			fileExists
			book {
				id
				resolvedName
				extension
				size
				thumbnail {
					url
				}
				metadata {
					writers
				}
			}
		}
	}
`)

/** How long removed books are kept, the trash folder and the automatic purge */
export const deletedBooksSettingsQuery = graphql(`
	query DeletedBooksSettings {
		deletedBooksSettings {
			retentionDays
			trashDir
			heldFiles
			heldBytes
			nextPurgeAt
			lastPurgeAt
			lastPurge {
				finishedAt
				jobId
				purged
				missingFiles
				failed
				originals
				errors {
					mediaId
					title
					error
				}
			}
		}
	}
`)

/** The books deleted permanently, latest first (a log: their files were deleted) */
export const purgedBooksQuery = graphql(`
	query PurgedBooks($limit: Int!) {
		purgedBooks(limit: $limit) {
			id
			mediaId
			title
			authors
			libraryName
			originalPath
			bytes
			purgedBy
			automatic
			purgedAt
		}
	}
`)

/** The files Stump holds for the retention: originals replaced by post-processing */
export const heldFilesQuery = graphql(`
	query HeldFiles($libraryId: ID) {
		heldFiles(libraryId: $libraryId) {
			id
			reason
			steps
			title
			libraryId
			libraryName
			relativePath
			bytes
			heldAt
			heldBy
			expiresAt
			purgeAt
			canPutBack
			book {
				id
				resolvedName
				thumbnail {
					url
				}
			}
		}
	}
`)

export const putBackHeldFileMutation = graphql(`
	mutation PutBackHeldFile($id: Int!) {
		putBackHeldFile(id: $id) {
			book {
				id
				resolvedName
			}
			replaced
		}
	}
`)

export const deleteHeldFilesMutation = graphql(`
	mutation DeleteHeldFiles($ids: [Int!]!) {
		deleteHeldFiles(ids: $ids) {
			deleted
			freedBytes
			kept {
				id
				title
				error
			}
		}
	}
`)

/** The libraries for the filter */
export const deletedBooksLibrariesQuery = graphql(`
	query DeletedBooksLibraries {
		libraries(pagination: { none: { unpaginated: true } }) {
			nodes {
				id
				name
			}
		}
	}
`)

export const restoreDeletedBooksMutation = graphql(`
	mutation RestoreDeletedBooks($ids: [ID!]!) {
		restoreDeletedBooks(ids: $ids)
	}
`)

export const purgeDeletedBooksMutation = graphql(`
	mutation PurgeDeletedBooks($ids: [ID!]!) {
		purgeDeletedBooks(ids: $ids) {
			purged
			missingFiles
			freedBytes
			kept {
				mediaId
				title
				error
			}
		}
	}
`)

export const emptyDeletedBooksMutation = graphql(`
	mutation EmptyDeletedBooks($libraryId: ID) {
		emptyDeletedBooks(libraryId: $libraryId) {
			purged
			missingFiles
			freedBytes
			kept {
				mediaId
				title
				error
			}
		}
	}
`)

export const updateRetentionMutation = graphql(`
	mutation UpdateDeletedBooksRetention($days: Int!) {
		updateDeletedBooksRetention(days: $days) {
			retentionDays
		}
	}
`)
