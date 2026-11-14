/**
 * The items of a list from the API (genres, authors, characters, ...) without the blank ones. A
 * blank item is no value: shown as a chip it is empty, and its link would filter on nothing. The
 * server reads these lists without blank items; this keeps a screen right when a server that does
 * not (yet) is behind it.
 */
export function nonBlank(
	items: readonly (string | null | undefined)[] | null | undefined,
): string[] {
	return (items ?? []).map((item) => item?.trim() ?? '').filter(Boolean)
}
