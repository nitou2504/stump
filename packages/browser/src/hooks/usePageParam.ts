import { useMemo } from 'react'
import { useSearchParams } from 'react-router'

/**
 * A hook to get and set the current page from the URL. This is useful for pagination.
 */
export function usePageParam() {
	const [search, setSearchParams] = useSearchParams()

	const page = useMemo(() => {
		const searchPage = search.get('page')
		if (searchPage) {
			return parseInt(searchPage, 10)
		}
		return 1
	}, [search])

	const setPage = (newPage: number) => {
		// no history entry for "changing" to the current page
		if (newPage === page) return
		setSearchParams((prev) => {
			prev.set('page', newPage.toString())
			return prev
		})
	}

	return { page, setPage }
}
