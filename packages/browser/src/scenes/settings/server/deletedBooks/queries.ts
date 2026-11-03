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

/** How long removed books are kept, the quarantine folder and the automatic purge */
export const deletedBooksSettingsQuery = graphql(`
	query DeletedBooksSettings {
		deletedBooksSettings {
			retentionDays
			quarantineDir
			quarantineReady
			nextPurgeAt
			lastPurgeAt
			lastPurge {
				finishedAt
				jobId
				purged
				missingFiles
				failed
				errors {
					mediaId
					title
					error
				}
			}
		}
	}
`)

/** The books deleted permanently, latest first */
export const purgedBooksQuery = graphql(`
	query PurgedBooks($limit: Int!) {
		purgedBooks(limit: $limit) {
			id
			mediaId
			title
			authors
			libraryName
			originalPath
			quarantinePath
			bytes
			purgedBy
			automatic
			purgedAt
			restoredAt
			restoredBy
			canRestore
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
			quarantinedBytes
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
			quarantinedBytes
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

export const restorePurgedBookMutation = graphql(`
	mutation RestorePurgedBook($id: Int!) {
		restorePurgedBook(id: $id) {
			id
			title
			libraryName
			restoredAt
		}
	}
`)
