import { graphql } from '@stump/graphql'

/** The values a metadata field has across the library, with how many books carry each */
export const metadataVocabularyQuery = graphql(`
	query MetadataVocabulary($field: MetadataFacetField!, $libraryId: ID, $search: String) {
		metadataValueCounts(field: $field, libraryId: $libraryId, search: $search) {
			value
			count
		}
	}
`)

/** Tags proposed for a book already in the library, and how its current ones fit */
export const mediaGenreSuggestionsQuery = graphql(`
	query MediaGenreSuggestions($id: ID!, $online: Boolean) {
		mediaGenreSuggestions(id: $id, online: $online) {
			suggestions {
				value
				confidence
				reasons
				libraryCount
				selected
				kind
			}
			current {
				value
				kind
				confidence
				fits
				reasons
				libraryCount
			}
			similarBooks {
				mediaId
				title
				authors
				genres
				reason
			}
			subjects
			lookedUpOnline
			onlineLookups {
				source
				status
				subjects
				message
			}
		}
	}
`)

/** Descriptions found online for a book (Hardcover first), for its summary */
export const mediaDescriptionSuggestionsQuery = graphql(`
	query MediaDescriptionSuggestions($id: ID!) {
		mediaDescriptionSuggestions(id: $id) {
			status
			candidates {
				source
				text
				words
			}
			onlineLookups {
				source
				status
				subjects
				message
			}
			searchedTitle
			searchedAuthors
		}
	}
`)

/** Saves a book's metadata and writes the changes into its file */
export const editMediaMetadataMutation = graphql(`
	mutation EditMediaMetadata($id: ID!, $input: MediaMetadataInput!) {
		editMediaMetadata(id: $id, input: $input) {
			media {
				id
			}
			file {
				status
				fileFields
				stumpOnlyFields
				message
			}
			saveId
		}
	}
`)

/** Puts back the values a save of the edit history changed */
export const revertMediaMetadataEditMutation = graphql(`
	mutation RevertMediaMetadataEdit($mediaId: ID!, $saveId: String!) {
		revertMediaMetadataEdit(mediaId: $mediaId, saveId: $saveId) {
			media {
				id
			}
			file {
				status
				fileFields
				stumpOnlyFields
				message
			}
			saveId
			skippedFields
		}
	}
`)

/** A book's latest metadata saves, newest first */
export const mediaMetadataEditsQuery = graphql(`
	query MediaMetadataEdits($mediaId: ID!, $limit: Int) {
		mediaMetadataEdits(mediaId: $mediaId, limit: $limit) {
			saveId
			editedAt
			editedBy
			revertsSaveId
			revertedBy
			changes {
				field
				oldValue
				newValue
				writtenToFile
			}
		}
	}
`)

/** What the "Edit details" dialog shows and edits */
export const editBookDetailsQuery = graphql(`
	query EditBookDetails($id: ID!) {
		mediaById(id: $id) {
			id
			resolvedName
			extension
			library {
				id
			}
			metadata {
				title
				writers
				series
				number
				genres
				summary
				publisher
				year
				month
				day
				language
				ageRating
			}
		}
	}
`)
