import { MetadataFileWriteStatus } from '@stump/graphql'
import { toast } from 'sonner'

/** What a save reports about the book file */
export type FileOutcome = {
	status: MetadataFileWriteStatus
	fileFields: string[]
	stumpOnlyFields: string[]
	message?: string | null
}

/** The editor's names for the metadata fields (camelCase, as the server reports them) */
export const FIELD_LABELS: Record<string, string> = {
	ageRating: 'age',
	characters: 'characters',
	colorists: 'colorists',
	coverArtists: 'cover artists',
	day: 'date',
	editors: 'editors',
	format: 'format',
	genres: 'genres',
	identifierAmazon: 'Amazon ID',
	identifierCalibre: 'Calibre ID',
	identifierGoogle: 'Google ID',
	identifierIsbn: 'ISBN',
	identifierMobiAsin: 'ASIN',
	identifierUuid: 'UUID',
	inkers: 'inkers',
	language: 'language',
	letterers: 'letterers',
	links: 'links',
	month: 'date',
	notes: 'notes',
	number: 'number',
	pageCount: 'page count',
	pencillers: 'pencillers',
	publisher: 'publisher',
	series: 'series',
	seriesGroup: 'series group',
	storyArc: 'story arc',
	storyArcNumber: 'story arc number',
	summary: 'summary',
	teams: 'teams',
	title: 'title',
	titleSort: 'sort title',
	volume: 'volume',
	writers: 'authors',
	year: 'date',
}

export const fieldLabel = (field: string) => FIELD_LABELS[field] ?? field

/** `["genres", "writers", "month", "day"]` → `genres, authors, date` */
export const describeFields = (fields: string[]) =>
	Array.from(new Set(fields.map(fieldLabel))).join(', ')

/** A server message as a sentence */
const sentence = (text: string) => (/[.!?]$/.test(text.trim()) ? text.trim() : `${text.trim()}.`)

/**
 * Tells where a saved (or reverted) edit went: into the book file, or into Stump only and
 * why. `skipped` lists the fields a revert left alone because they changed again since.
 */
export function notifySaved(
	file: FileOutcome,
	{ reverted = false, skipped = [] }: { reverted?: boolean; skipped?: string[] } = {},
) {
	const verb = reverted ? 'Reverted' : 'Saved'
	const stumpOnly = describeFields(file.stumpOnlyFields)
	const left = skipped.length ? `Not reverted (changed since): ${describeFields(skipped)}.` : ''
	const join = (...parts: (string | false | null | undefined)[]) =>
		parts.filter(Boolean).join(' ') || undefined

	if (file.status === MetadataFileWriteStatus.Written) {
		toast.success(`${verb} to the book file`, {
			description: join(stumpOnly && `In Stump only: ${stumpOnly}.`, left),
		})
	} else if (file.status === MetadataFileWriteStatus.Failed) {
		toast.warning(`${verb} in Stump only`, {
			description: join(sentence(file.message ?? 'The book file was not changed'), left),
			duration: 12000,
		})
	} else if (!file.fileFields.length && !file.stumpOnlyFields.length) {
		toast.info('Nothing changed')
	} else {
		toast.success(`${verb} in Stump`, {
			description: join(
				file.message
					? sentence(file.message)
					: stumpOnly && `The book file has no place for ${stumpOnly}.`,
				left,
			),
		})
	}
}
