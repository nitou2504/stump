import { FIELD_LABELS } from './outcome'

/** The metadata fields stored as comma-separated lists */
const LIST_FIELDS = new Set([
	'characters',
	'colorists',
	'coverArtists',
	'editors',
	'genres',
	'inkers',
	'letterers',
	'links',
	'pencillers',
	'teams',
	'writers',
])

export const isListField = (field: string) => LIST_FIELDS.has(field)

/** A list value as stored (`A, B, C`) as its items */
export const listItems = (value?: string | null) =>
	(value ?? '')
		.split(',')
		.map((item) => item.trim())
		.filter(Boolean)

/** What a change did to a list: the items it removed and the ones it added */
export function listDiff(oldValue?: string | null, newValue?: string | null) {
	const before = listItems(oldValue)
	const after = listItems(newValue)
	const key = (item: string) => item.toLowerCase()
	const had = new Set(before.map(key))
	const has = new Set(after.map(key))
	return {
		added: after.filter((item) => !had.has(key(item))),
		removed: before.filter((item) => !has.has(key(item))),
	}
}

/** A field's name in the history (a date's parts by name) */
export function historyLabel(field: string) {
	if (field === 'year' || field === 'month' || field === 'day') return field
	return FIELD_LABELS[field] ?? field
}

type ChangeLike = { writtenToFile: boolean }

/** Where a save went: all into the file, all into Stump only, or some of each */
export function saveDestination(changes: ChangeLike[]): 'file' | 'stump' | 'both' {
	const written = changes.filter((c) => c.writtenToFile).length
	if (written === changes.length) return 'file'
	if (written === 0) return 'stump'
	return 'both'
}
