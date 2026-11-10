import { graphql, IntakeItemFieldsFragment, IntakeUploadLimitsQuery } from '@stump/graphql'

/**
 * Everything the inbox shows of an item: the same for the user's own inbox and for the queue of
 * books waiting for approval. Read it back with `useFragment(intakeItemFragment, ...)`
 */
export const intakeItemFragment = graphql(`
	fragment IntakeItemFields on IntakeItem {
		id
		libraryId
		libraryName
		libraryType
		suggestedLibrary {
			id
			name
		}
		state
		kind
		name
		error
		createdAt
		filedAt
		submittedAt
		reviewedAt
		reviewNote
		uploader {
			id
			username
			avatarUrl
		}
		problems
		mediaIds
		files {
			originalName
			size
			format
			volume
			title
		}
		proposal {
			metadata {
				title
				authors
				series
				seriesIndex
				language
				year
				publisher
				description
				isbn
				subjects
				genres
				sources {
					field
					source
				}
			}
			seriesKind
			authorFolders {
				folder
				books
				confidence
				reason
				isNew
			}
			seriesFolders {
				folder
				books
				confidence
				reason
				isNew
			}
			genreSuggestions {
				value
				confidence
				reasons
				libraryCount
				selected
				kind
			}
			similarBooks {
				mediaId
				title
				authors
				genres
				reason
			}
			duplicates {
				mediaId
				path
				title
				reason
				fileName
			}
			warnings
			confidence
			lookedUpOnline
		}
		decision {
			metadata {
				title
				authors
				series
				seriesIndex
				language
				year
				publisher
				description
				genres
				ageRating
				sources {
					field
					source
				}
			}
			seriesKind
			folder
			fileNames
			volumes
		}
		destination {
			relativeDir
			dirExists
			files {
				originalName
				relativePath
				exists
			}
		}
		result {
			paths
			notes
		}
	}
`)

/** The user's inbox, newest first; everything but discarded items */
export const intakeItemsQuery = graphql(`
	query IntakeItems {
		intakeItems {
			...IntakeItemFields
		}
	}
`)

/** An inbox item, as the inbox and the approval queue both show it */
export type IntakeItemData = IntakeItemFieldsFragment

export const intakeLibrariesQuery = graphql(`
	query IntakeLibraries {
		intakeLibraries {
			id
			name
		}
	}
`)

export const intakeVocabularyQuery = graphql(`
	query IntakeVocabulary($libraryId: ID!, $search: String, $limit: Int! = 20) {
		intakeVocabulary(libraryId: $libraryId, search: $search, limit: $limit) {
			value
			count
			kind
		}
	}
`)

export const intakeFoldersQuery = graphql(`
	query IntakeFolders(
		$libraryId: ID!
		$seriesKind: IntakeSeriesKind
		$search: String
		$limit: Int! = 20
	) {
		intakeFolders(libraryId: $libraryId, seriesKind: $seriesKind, search: $search, limit: $limit) {
			name
			books
		}
	}
`)

export const intakeReviewMutation = graphql(`
	mutation IntakeReview($id: ID!, $input: IntakeReviewInput!) {
		intakeReview(id: $id, input: $input) {
			id
			state
		}
	}
`)

export const intakeReanalyseMutation = graphql(`
	mutation IntakeReanalyse($id: ID!, $online: Boolean) {
		intakeReanalyse(id: $id, online: $online) {
			id
			state
		}
	}
`)

export const intakeChangeLibraryMutation = graphql(`
	mutation IntakeChangeLibrary($id: ID!, $libraryId: ID!) {
		intakeChangeLibrary(id: $id, libraryId: $libraryId) {
			id
			state
			libraryId
		}
	}
`)

export const intakeAcceptMutation = graphql(`
	mutation IntakeAccept($ids: [ID!]!) {
		intakeAccept(ids: $ids) {
			id
			state
			error
		}
	}
`)

export const intakeDiscardMutation = graphql(`
	mutation IntakeDiscard($ids: [ID!]!) {
		intakeDiscard(ids: $ids)
	}
`)

/** What the viewer may upload, how much of it they have used, and whether uploads are open */
export const intakeUploadLimitsQuery = graphql(`
	query IntakeUploadLimits {
		intakeUploadLimits {
			maxRequestBytes
			maxFileBytes
			maxFilesPerRequest
			quotaBytes
			usedBytes
			maxItems
			openItems
			quotaApplies
			canFileDirectly
			uploadsOpen
			closedReason
		}
	}
`)

export type IntakeLimitsData = IntakeUploadLimitsQuery['intakeUploadLimits']

/** The books waiting for approval in the libraries the viewer can use, oldest first */
export const pendingIntakeItemsQuery = graphql(`
	query PendingIntakeItems($libraryId: ID, $page: Int! = 1, $pageSize: Int! = 20) {
		pendingIntakeItems(libraryId: $libraryId, page: $page, pageSize: $pageSize) {
			totalCount
			nodes {
				...IntakeItemFields
			}
		}
	}
`)

export const pendingIntakeCountQuery = graphql(`
	query PendingIntakeCount {
		pendingIntakeCount
	}
`)

/** The age limit of a person, for the warning that a rating would hide a book from them */
export const intakeUploaderAgeQuery = graphql(`
	query IntakeUploaderAge($userId: ID!) {
		userById(id: $userId) {
			id
			ageRestriction {
				age
				restrictOnUnset
			}
		}
	}
`)

export const intakeSubmitMutation = graphql(`
	mutation IntakeSubmit($ids: [ID!]!) {
		intakeSubmit(ids: $ids) {
			id
			state
		}
	}
`)

export const intakeWithdrawMutation = graphql(`
	mutation IntakeWithdraw($ids: [ID!]!) {
		intakeWithdraw(ids: $ids) {
			id
			state
		}
	}
`)

export const intakeApproveMutation = graphql(`
	mutation IntakeApprove($ids: [ID!]!) {
		intakeApprove(ids: $ids) {
			id
			state
			error
		}
	}
`)

export const intakeRejectMutation = graphql(`
	mutation IntakeReject($id: ID!, $note: String!) {
		intakeReject(id: $id, note: $note) {
			id
			state
			reviewNote
		}
	}
`)
