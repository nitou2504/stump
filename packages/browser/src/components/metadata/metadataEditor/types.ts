import { MediaMetadataEditorFragment, SeriesMetadataEditorFragment } from '@stump/graphql'

export type MetadataEditorRow<Field extends string> = {
	label: string
	field: Field
}

export type MediaMetadataField = keyof Omit<
	MediaMetadataEditorFragment,
	'__typename' | ' $fragmentName' | 'mediaId'
>

export type MediaMetadataArrayField = Extract<
	MediaMetadataField,
	| 'characters'
	| 'colorists'
	| 'coverArtists'
	| 'editors'
	| 'genres'
	| 'inkers'
	| 'letterers'
	| 'links'
	| 'pencillers'
	| 'teams'
	| 'writers'
>

export type MediaMetadataEditorRow = MetadataEditorRow<MediaMetadataField>

export const MediaMetadataKeys: MediaMetadataField[] = [
	// what a book is, in the order people look for it
	'title',
	'writers',
	'series',
	'number',
	'genres',
	'summary',
	'publisher',
	'year',
	'month',
	'day',
	'language',
	'ageRating',
	// then the rest
	'titleSort',
	'volume',
	'seriesGroup',
	'storyArc',
	'storyArcNumber',
	'format',
	'editors',
	'coverArtists',
	'pencillers',
	'inkers',
	'colorists',
	'letterers',
	'characters',
	'teams',
	'links',
	'notes',
	'pageCount',
	'identifierAmazon',
	'identifierCalibre',
	'identifierGoogle',
	'identifierIsbn',
	'identifierMobiAsin',
	'identifierUuid',
]

export type SeriesMetadataField = keyof Omit<
	SeriesMetadataEditorFragment,
	'__typename' | ' $fragmentName' | 'seriesId'
>

export type SeriesMetadataEditorRow = MetadataEditorRow<SeriesMetadataField>

export const SeriesMetadataKeys: SeriesMetadataField[] = [
	'title',
	'summary',
	'comicid',
	'volume',
	'year',
	'totalIssues',
	'publisher',
	'booktype',
	'ageRating',
	'writers',
	'characters',
	'genres',
	'imprint',
	'links',
	'metaType',
	'status',
	'publicationRun',
	'comicImage',
	'descriptionFormatted',
]
