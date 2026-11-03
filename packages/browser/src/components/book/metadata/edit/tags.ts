/**
 * Helpers for editing a book's tags (genres) against the library's own vocabulary. The
 * curated library writes three kinds of tags: big genres (`Philosophy`), small ones under a
 * big one (`Philosophy: Ethics`) and facets (`Origin: French`, `Level: General Reader`, ...),
 * mirroring `core/src/intake/library_index.rs` and `write.rs` on the server.
 */

/** Tag prefixes the curated library uses for facets (not genres) */
export const FACET_PREFIXES = [
	'Origin',
	'Era',
	'Level',
	'Audience',
	'Form',
	'Award',
	'Laureate',
	'Canon',
	'Course',
	'Collection',
	'Source',
]

export type TagGroup = 'genre' | 'subgenre' | 'facet'

export function isFacet(tag: string): boolean {
	if (tag === 'Illustrated') return true
	const at = tag.indexOf(':')
	return at > 0 && FACET_PREFIXES.includes(tag.slice(0, at).trim())
}

export function tagGroup(tag: string): TagGroup {
	if (isFacet(tag)) return 'facet'
	return tag.includes(':') ? 'subgenre' : 'genre'
}

/** Big tags, then `Big: Small` tags, then facets, keeping the order within each group */
export function orderTags(tags: string[]): string[] {
	const unique = Array.from(new Set(tags.map((t) => t.trim()).filter(Boolean)))
	const rank = { facet: 2, genre: 0, subgenre: 1 }
	return unique
		.map((tag, index) => ({ index, rank: rank[tagGroup(tag)], tag }))
		.sort((a, b) => a.rank - b.rank || a.index - b.index)
		.map(({ tag }) => tag)
}

/** Lowercase without accents, for matching `filosofia` to `Filosofía` */
export function fold(text: string): string {
	return text
		.normalize('NFD')
		.replace(/\p{Diacritic}/gu, '')
		.toLowerCase()
}

/** Every word of the search starts a word of the value, in any order (like the fork's search) */
export function matchesSearch(value: string, search: string): boolean {
	const words = fold(search)
		.split(/[^\p{L}\p{N}]+/u)
		.filter(Boolean)
	if (!words.length) return true
	const valueWords = fold(value)
		.split(/[^\p{L}\p{N}]+/u)
		.filter(Boolean)
	return words.every((word) => valueWords.some((v) => v.startsWith(word)))
}

export type VocabEntry = { value: string; count: number }

export type GroupedVocabulary = Record<TagGroup, VocabEntry[]>

/**
 * The library's tags matching a search, grouped and most used first, without the ones the
 * book already has. `limit` caps each group.
 */
export function groupVocabulary(
	entries: VocabEntry[],
	search: string,
	exclude: string[],
	limit = 40,
): GroupedVocabulary {
	const taken = new Set(exclude.map((e) => e.toLowerCase()))
	const out: GroupedVocabulary = { facet: [], genre: [], subgenre: [] }
	const sorted = [...entries].sort((a, b) => b.count - a.count || a.value.localeCompare(b.value))
	for (const entry of sorted) {
		if (taken.has(entry.value.toLowerCase()) || !matchesSearch(entry.value, search)) continue
		const group = out[tagGroup(entry.value)]
		if (group.length < limit) group.push(entry)
	}
	return out
}

/**
 * A typed value cleaned up for use as a new tag or name: single spaces, no commas or
 * semicolons (they separate values). `null` when nothing is left.
 */
export function cleanValue(text: string): string | null {
	const cleaned = text.replace(/[,;]/g, ' ').replace(/\s+/g, ' ').trim()
	return cleaned || null
}

/** The value a "create" option would add, or `null` when it already exists or is chosen */
export function creatableValue(
	search: string,
	entries: VocabEntry[],
	chosen: string[],
): string | null {
	const value = cleanValue(search)
	if (!value) return null
	const key = value.toLowerCase()
	const exists = [...entries.map((e) => e.value), ...chosen].some((v) => v.toLowerCase() === key)
	return exists ? null : value
}

export type SuggestionLike = { value: string; selected: boolean; kind: string }
export type CurrentFitLike = { value: string; fits: boolean; kind: string }

/**
 * The default picks of a suggestion run: drop the current genres nothing supports
 * (facets are kept unless removed by hand), add the server's default choice
 */
export function defaultPicks(
	current: string[],
	fits: CurrentFitLike[],
	suggestions: SuggestionLike[],
): { remove: string[]; add: string[] } {
	const remove = fits
		.filter((f) => !f.fits && f.kind !== 'FACET' && current.includes(f.value))
		.map((f) => f.value)
	const have = new Set(current.map((c) => c.toLowerCase()))
	const add = suggestions
		.filter((s) => s.selected && !have.has(s.value.toLowerCase()))
		.map((s) => s.value)
	return { add, remove }
}

/** The tags after applying picks: removed ones out, added ones in, in the library's order */
export function applyPicks(current: string[], remove: string[], add: string[]): string[] {
	const removed = new Set(remove.map((r) => r.toLowerCase()))
	return orderTags([...current.filter((c) => !removed.has(c.toLowerCase())), ...add])
}
