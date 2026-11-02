import type { PdfLayout, PdfOutlineEntry } from '@stump/sdk'

export type FlatOutlineEntry = {
	title: string
	page: number | null
	depth: number
}

/** The outline as a flat list in reading order, with each entry's depth */
export function flattenOutline(entries: PdfOutlineEntry[], depth = 0): FlatOutlineEntry[] {
	return entries.flatMap((entry) => [
		{ title: entry.title, page: entry.page, depth },
		...flattenOutline(entry.children ?? [], depth + 1),
	])
}

/**
 * The title of the innermost outline entry that contains a page: the last entry (in
 * reading order) starting at or before it, the deepest one when several start on the same
 * page. Empty when the outline has none.
 */
export function chapterTitleForPage(flat: FlatOutlineEntry[], page: number): string {
	let best: FlatOutlineEntry | null = null
	for (const entry of flat) {
		if (entry.page == null || entry.page > page) continue
		if (
			!best ||
			entry.page > best.page! ||
			(entry.page === best.page && entry.depth >= best.depth)
		) {
			best = entry
		}
	}
	return best?.title ?? ''
}

/** The label to show for a 1-based page: the PDF's own label, or the number */
export function pageLabel(
	layout: Pick<PdfLayout, 'labels'> | null | undefined,
	page: number,
): string {
	const label = layout?.labels?.[page - 1]
	return label && label.trim() ? label : String(page)
}

/**
 * Resolves what the user typed in "Go to page": a page label first (e.g. "xix"), then a page
 * number. `null` when it matches nothing.
 */
export function resolvePageInput(
	layout: Pick<PdfLayout, 'labels' | 'pageCount'>,
	input: string,
): number | null {
	const value = input.trim()
	if (!value) return null
	const labelIndex = layout.labels?.findIndex(
		(label) => label.trim().toLowerCase() === value.toLowerCase(),
	)
	if (labelIndex != null && labelIndex >= 0) return labelIndex + 1
	if (!/^\d+$/.test(value)) return null
	const page = Number(value)
	return page >= 1 && page <= layout.pageCount ? page : null
}
