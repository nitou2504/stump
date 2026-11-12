import { formatBytes } from '@stump/client'

export type BookCardDetail = {
	/** What the card says under the title */
	text: string
	/** The full text, when `text` may be cut short to fit the card */
	title?: string
}

/**
 * What a book card says under its title while the book is not started: its authors (the first
 * ones show, the card cuts the rest), or its file size when no author is known (a PDF with no
 * metadata, say). The size stays on the book's page.
 */
export function bookCardDetail(
	writers: readonly (string | null | undefined)[] | null | undefined,
	size: number,
): BookCardDetail {
	const authors = [
		...new Set(
			(writers ?? [])
				.map((writer) => writer?.trim())
				.filter((writer): writer is string => !!writer),
		),
	]
	if (!authors.length) {
		return { text: formatBytes(size) ?? '' }
	}

	const text = authors.join(', ')
	return { text, title: text }
}
