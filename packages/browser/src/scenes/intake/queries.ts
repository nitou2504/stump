import { graphql, IntakeItemsQuery } from '@stump/graphql'

/** The user's inbox, newest first; everything but discarded items */
export const intakeItemsQuery = graphql(`
	query IntakeItems {
		intakeItems {
			id
			libraryId
			libraryName
			state
			kind
			name
			error
			createdAt
			filedAt
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
	}
`)

export type IntakeItemData = IntakeItemsQuery['intakeItems'][number]

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

export const intakeUploadMutation = graphql(`
	mutation IntakeUpload(
		$libraryId: ID!
		$files: [Upload!]!
		$relativePaths: [String!]
		$mode: IntakeUploadMode! = AUTO
	) {
		intakeUpload(libraryId: $libraryId, files: $files, relativePaths: $relativePaths, mode: $mode) {
			id
			state
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
