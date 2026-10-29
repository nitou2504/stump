import { Media } from '@stump/graphql'

/**
 * Viewer-side groupings of a smart list's books by one of their details, computed in the
 * browser from the list's (ungrouped) books. `pattern` groups by the `*` part of genres
 * and tags shaped like `Asignatura: *`, so any "Key: value" tagging scheme works without
 * server support.
 */
export type GroupField =
	| 'writers'
	| 'genres'
	| 'language'
	| 'publisher'
	| 'year'
	| 'decade'
	| 'rating'
	| 'ageRating'
	| 'pattern'

export type FieldGrouping =
	| { field: Exclude<GroupField, 'pattern'> }
	| { field: 'pattern'; pattern: string }

export type BookFieldGroup = {
	/** Stable key, unique within the grouping */
	key: string
	name: string
	books: Media[]
}

export type FieldGroupingResult = {
	groups: BookFieldGroup[]
	/** Books with no value for the field */
	ungrouped: Media[]
	/** Whether some books appear in more than one group (e.g. several authors) */
	hasMultiMembership: boolean
}

export const FIELD_LABELS: Record<Exclude<GroupField, 'pattern'>, string> = {
	writers: 'Author',
	genres: 'Genre / tag',
	language: 'Language',
	publisher: 'Publisher',
	year: 'Year',
	decade: 'Decade',
	rating: 'Rating',
	ageRating: 'Age rating',
}

export const GROUP_FIELDS = Object.keys(FIELD_LABELS) as Exclude<GroupField, 'pattern'>[]

/** `Key: value` genres/tags. The key is short and has no `=` (skips `bcurator:age=...`) */
const KEYED_VALUE = /^([^:=]{1,40}?)\s*:\s*([^=]+)$/
const HIDDEN_PREFIXES = new Set(['bcurator'])

const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: 'base' })

/** The label of a grouping, e.g. `Author` or `Asignatura` for `Asignatura: *` */
export function groupingLabel(grouping: FieldGrouping): string {
	if (grouping.field !== 'pattern') return FIELD_LABELS[grouping.field]
	const prefix = grouping.pattern
		.split('*')[0]
		?.replace(/[\s:·\-–|/]+$/, '')
		.trim()
	return prefix || grouping.pattern
}

export function isSameGrouping(a?: FieldGrouping | null, b?: FieldGrouping | null): boolean {
	if (!a || !b) return !a && !b
	if (a.field !== b.field) return false
	if (a.field === 'pattern' && b.field === 'pattern') {
		return a.pattern.trim().toLowerCase() === b.pattern.trim().toLowerCase()
	}
	return true
}

const escapeRegExp = (value: string) => value.replace(/[.+?^${}()|[\]\\]/g, '\\$&')

/**
 * Turns `Asignatura: *` into a matcher returning the `*` part. Without a `*` the pattern
 * is a prefix (`Asignatura` behaves like `Asignatura*`), and separators after the prefix
 * are trimmed from the captured value. Matching ignores case and surrounding spaces.
 */
export function compilePattern(pattern: string): ((value: string) => string | null) | null {
	const trimmed = pattern.trim()
	if (!trimmed || trimmed === '*') return null
	const withWildcard = trimmed.includes('*') ? trimmed : `${trimmed}*`
	const source = withWildcard
		.split('*')
		.map((part) => escapeRegExp(part).replace(/\s+/g, '\\s*'))
		.join('(.+?)')
	const regex = new RegExp(`^\\s*${source}\\s*$`, 'i')
	return (value: string) => {
		const match = regex.exec(value)
		if (!match) return null
		const captured = match
			.slice(1)
			.join(' ')
			.replace(/^[\s:·\-–|/]+/, '')
			.trim()
		return captured || null
	}
}

/** A book's genres and tags, deduplicated */
function labelsOf(book: Media): string[] {
	const genres = book.metadata?.genres ?? []
	const tags = (book.tags ?? []).map((tag) => tag.name)
	return [...new Set([...genres, ...tags].map((value) => value.trim()).filter(Boolean))]
}

let languageNames: Intl.DisplayNames | null | undefined
function languageName(code: string): string {
	const normalized = code.trim().toLowerCase().split(/[-_]/)[0] ?? ''
	if (languageNames === undefined) {
		try {
			languageNames = new Intl.DisplayNames(undefined, { type: 'language' })
		} catch {
			languageNames = null
		}
	}
	try {
		const name = languageNames?.of(normalized)
		if (name && name.toLowerCase() !== normalized) {
			return name.charAt(0).toUpperCase() + name.slice(1)
		}
	} catch {
		// not a valid language code, fall through
	}
	return code
}

const RATING_BUCKETS: Array<{ min: number; key: string; name: string }> = [
	{ min: 4.5, key: '4.5', name: '★ 4.5 – 5' },
	{ min: 4, key: '4', name: '★ 4 – 4.5' },
	{ min: 3.5, key: '3.5', name: '★ 3.5 – 4' },
	{ min: 3, key: '3', name: '★ 3 – 3.5' },
	{ min: 0, key: '0', name: '★ below 3' },
]

/** The group keys and names a book belongs to for a grouping */
function valuesOf(
	book: Media,
	grouping: FieldGrouping,
	matcher: ((value: string) => string | null) | null,
): Array<{ key: string; name: string }> {
	const metadata = book.metadata
	const text = (values: Array<string | null | undefined>) =>
		values
			.map((value) => value?.trim())
			.filter((value): value is string => !!value)
			.map((value) => ({ key: value.toLowerCase(), name: value }))

	switch (grouping.field) {
		case 'writers':
			return text(metadata?.writers ?? [])
		case 'genres':
			return text(
				labelsOf(book).filter((value) => {
					const keyed = KEYED_VALUE.exec(value)
					return !keyed && !value.includes('=')
				}),
			)
		case 'publisher':
			return text([metadata?.publisher])
		case 'language': {
			const code = metadata?.language?.trim()
			if (!code) return []
			const key = code.toLowerCase().split(/[-_]/)[0] ?? code
			return [{ key, name: languageName(code) }]
		}
		case 'year':
			return metadata?.year ? [{ key: String(metadata.year), name: String(metadata.year) }] : []
		case 'decade': {
			if (!metadata?.year) return []
			const decade = Math.floor(metadata.year / 10) * 10
			return [{ key: String(decade), name: `${decade}s` }]
		}
		case 'rating': {
			const rating = metadata?.rating
			if (rating == null || rating <= 0) return []
			const bucket = RATING_BUCKETS.find(({ min }) => rating >= min)
			return bucket ? [{ key: bucket.key, name: bucket.name }] : []
		}
		case 'ageRating': {
			const age = metadata?.ageRating
			if (age == null) return []
			return [{ key: String(age), name: age > 0 ? `${age}+` : 'All ages' }]
		}
		case 'pattern':
			if (!matcher) return []
			return text(labelsOf(book).map(matcher))
	}
}

/** Group order: best rating first, ages and years ascending, otherwise by name */
function compareGroups(field: GroupField) {
	return (a: BookFieldGroup, b: BookFieldGroup): number => {
		if (field === 'rating' || field === 'ageRating' || field === 'year' || field === 'decade') {
			const diff = Number(a.key) - Number(b.key)
			return field === 'rating' ? -diff : diff
		}
		return collator.compare(a.name, b.name)
	}
}

export function groupBooksByField(books: Media[], grouping: FieldGrouping): FieldGroupingResult {
	const matcher = grouping.field === 'pattern' ? compilePattern(grouping.pattern) : null
	const groups = new Map<string, BookFieldGroup>()
	const ungrouped: Media[] = []
	let hasMultiMembership = false

	for (const book of books) {
		const values = valuesOf(book, grouping, matcher)
		const seen = new Set<string>()
		for (const { key, name } of values) {
			if (seen.has(key)) continue
			seen.add(key)
			const group = groups.get(key)
			if (group) {
				group.books.push(book)
			} else {
				groups.set(key, { books: [book], key, name })
			}
		}
		if (seen.size === 0) ungrouped.push(book)
		if (seen.size > 1) hasMultiMembership = true
	}

	return {
		groups: [...groups.values()].sort(compareGroups(grouping.field)),
		hasMultiMembership,
		ungrouped,
	}
}

export type DetectedPrefix = {
	/** e.g. `Asignatura` */
	prefix: string
	/** The pattern to group by it, e.g. `Asignatura: *` */
	pattern: string
	/** How many distinct values it has in these books */
	values: number
	/** How many of these books have it */
	books: number
}

/**
 * Finds `Key: value` schemes in the books' genres and tags (e.g. `Asignatura: Física`),
 * most common first, so they can be offered as one-click groupings.
 */
export function detectKeyedPrefixes(books: Media[]): DetectedPrefix[] {
	const found = new Map<string, { prefix: string; values: Set<string>; books: number }>()
	for (const book of books) {
		const prefixesInBook = new Set<string>()
		for (const value of labelsOf(book)) {
			const match = KEYED_VALUE.exec(value)
			if (!match?.[1] || !match[2]) continue
			const prefix = match[1].trim()
			const key = prefix.toLowerCase()
			if (HIDDEN_PREFIXES.has(key)) continue
			const entry = found.get(key) ?? { books: 0, prefix, values: new Set<string>() }
			entry.values.add(match[2].trim().toLowerCase())
			found.set(key, entry)
			prefixesInBook.add(key)
		}
		for (const key of prefixesInBook) {
			const entry = found.get(key)
			if (entry) entry.books += 1
		}
	}
	return [...found.values()]
		.map(({ prefix, values, books }) => ({
			books,
			pattern: `${prefix}: *`,
			prefix,
			values: values.size,
		}))
		.sort((a, b) => b.books - a.books || collator.compare(a.prefix, b.prefix))
}
