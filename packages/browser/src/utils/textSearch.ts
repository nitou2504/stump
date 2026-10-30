/**
 * Client-side text search matching the server's (see `crates/graphql/src/filter/search.rs`):
 * a query is split into words and every word must start a word in at least one of the given
 * texts, ignoring case and accents. `brown chemistry`, `quim` (→ `Química`) and `fisica`
 * work, while `ib` doesn't match inside `biblioteca`.
 */

/** Characters treated as spaces between words, like the server's `SEPARATORS` */
const SEPARATORS = /[\s_\-()[\]:,.;/"«»¿?¡!—–·|]+/gu

/** Lowercases and strips accents (`Física` → `fisica`) */
export const foldText = (value: string): string =>
	value.normalize('NFD').replace(/\p{M}/gu, '').toLowerCase()

/** The folded, unique words of a query */
export const searchWords = (query: string): string[] => [
	...new Set(
		foldText(query)
			.split(SEPARATORS)
			.map((word) => word.replace(/^[^\p{L}\p{N}]+|[^\p{L}\p{N}]+$/gu, ''))
			.filter(Boolean),
	),
]

/** Whether every word of `query` starts a word in some of `texts` (an empty query matches) */
export function matchesSearch(
	query: string,
	texts: Array<string | number | null | undefined>,
): boolean {
	const words = searchWords(query)
	if (!words.length) return true
	const haystack = ` ${texts
		.filter((text) => text != null && text !== '')
		.map((text) => foldText(String(text)).replace(SEPARATORS, ' '))
		.join(' ')}`
	return words.every((word) => haystack.includes(` ${word}`))
}
