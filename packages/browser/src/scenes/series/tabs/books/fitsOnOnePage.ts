type Args = {
	/** Whether the list has loaded: until it has, the page controls stay as they are */
	loaded: boolean
	totalPages: number
	/**
	 * The page being shown. A page past the end (a bigger page size kept the old ?page=) is an empty
	 * list that only its controls can leave, so they stay
	 */
	currentPage: number
	/** What narrows the list: while any of it is applied, the controls stay so it can be undone */
	search?: string
	startsWith?: string
	filters?: object
}

/**
 * Whether the whole list fits on one page, is showing that page and nothing narrows it, so the
 * search, the A–Z bar and the page controls have nothing to do (a six-book series needs no "1 of 1")
 */
export function fitsOnOnePage({
	loaded,
	totalPages,
	currentPage,
	search,
	startsWith,
	filters,
}: Args) {
	const isNarrowed = !!search || !!startsWith || Object.keys(filters ?? {}).length > 0
	return loaded && totalPages <= 1 && currentPage <= 1 && !isNarrowed
}
