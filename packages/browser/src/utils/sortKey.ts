/**
 * An accent- and case-insensitive key for sorting and matching names, so that e.g.
 * `Ética` sorts next to `Etnografía` and the search `etica` finds it
 */
export const sortKey = (value: string) =>
	value
		.normalize('NFD')
		.replace(/\p{Diacritic}/gu, '')
		.toLowerCase()
