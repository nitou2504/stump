/**
 * Types for the content REST API (`/api/v2/content/…`): the search inside the books' text.
 */

/** A hit of `GET /api/v2/content/search` in text mode: a passage of a book */
export type ContentHit = {
	media_id: string
	title: string
	authors: string[]
	language: string | null
	library: string
	tags: string[]
	score: number
	section: number | null
	section_title: string | null
	/** Character offset of the match in its section */
	pos: number | null
	/** Plain text around the match (no markup): the client marks the words */
	snippet: string | null
	/** For a PDF, the label of the page */
	page_label: string | null
}

export type ContentSearchResponse = {
	total: number
	page: number
	page_size: number
	items: ContentHit[]
}

export type ContentSearchParams = {
	q: string
	/** `any` of the words (default) or `all` */
	match?: 'any' | 'all'
	page?: number
	page_size?: number
	/** Only the books of this library */
	library_id?: string
	/** Only this book */
	media_id?: string
	signal?: AbortSignal
}

/** `GET /api/v2/content/status` */
export type ContentStatus = {
	books: number
	indexed: number
	pending: number
	failed: number
}
