import { ReactNode } from 'react'

const escape = (word: string) => word.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')

/**
 * The text with the words of `query` marked. The server's snippets are plain text, so the marks
 * are made here; accents are not matched across forms, which only means a word may go unmarked.
 */
export function highlight(text: string, query: string): ReactNode {
	const words = query
		.split(/\s+/)
		.map((w) => w.replace(/^["+-]+|["]+$/g, ''))
		.filter((w) => w.length > 1)
	if (!words.length) return text
	const pattern = new RegExp(`(${words.map(escape).join('|')})`, 'giu')
	return text.split(pattern).map((part, i) =>
		// split with one group puts the matches at the odd places
		i % 2 === 1 ? (
			<mark key={i} className="rounded-sm bg-transparent font-semibold text-foreground underline">
				{part}
			</mark>
		) : (
			part
		),
	)
}
