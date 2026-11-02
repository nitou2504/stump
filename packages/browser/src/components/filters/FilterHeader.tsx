import { cn, useSticky } from '@stump/components'
import { useCallback } from 'react'
import { useMediaMatch } from 'rooks'

import { useFilterContext } from './context'
import { ListToolbar, ListToolbarControls } from './ListToolbar'
import Search from './Search'

type Props = {
	/**
	 * Whether the search is being fetched from the server
	 */
	isSearching?: boolean
	/**
	 * Whether the search input should be disabled
	 */
	isSearchDisabled?: boolean
	/**
	 * Placeholder for the search input
	 */
	searchPlaceholder?: string
	/**
	 * The controls for adjusting the order of the items
	 */
	orderControls?: React.ReactNode
	/**
	 * The controls for adjusting the filters
	 */
	filterControls?: React.ReactNode
	/**
	 * The controls for adjusting the layout, i.e. GRID or TABLE
	 */
	layoutControls?: React.ReactNode
	/**
	 * The controls for adjusting the size of cards/items
	 */
	sizeControls?: React.ReactNode
	navOffset?: boolean
	/**
	 * A search box to show in place of the default one, which reads and writes the filter
	 * context's search: e.g. the home page's quick search, or a page that filters in place
	 */
	search?: React.ReactNode
}

// TODO: transparent until sticky hits, then bg-background
export default function FilterHeader({
	isSearching,
	isSearchDisabled,
	searchPlaceholder,
	layoutControls,
	orderControls,
	filterControls,
	sizeControls,
	navOffset,
	search: customSearch,
}: Props) {
	const isMobile = useMediaMatch('(max-width: 768px)')
	const { ref, isSticky } = useSticky<HTMLDivElement>({
		extraOffset: isMobile ? 56 : 0,
	})

	const { search, setSearch, removeSearch } = useFilterContext()
	const onSearchChange = useCallback(
		(value: string) => {
			if (value === search) return
			// Note: This prevents unnecessary state updates which causes unneeded location state
			// changes which causes an IRRITATING bug where you have to back out of the page like
			// 4-6 times to actually go back when using the browser back button lol
			if (!value && !search) return
			if (value) {
				setSearch(value)
			} else {
				removeSearch()
			}
		},
		[setSearch, removeSearch, search],
	)

	const hasControls = !!(orderControls || filterControls || layoutControls || sizeControls)

	return (
		<ListToolbar
			ref={ref}
			hasControls={hasControls}
			className={cn(
				'gap-2 h-12 px-4 md:top-0 sticky z-50 flex w-full shrink-0 items-center justify-between border-b border-border',
				{ 'bg-background': isSticky || !isMobile },
				navOffset ? 'top-12' : 'top-0',
			)}
		>
			{customSearch ?? (
				<Search
					initialValue={search || ''}
					placeholder={searchPlaceholder}
					onChange={onSearchChange}
					isLoading={isSearching}
					isDisabled={isSearchDisabled}
				/>
			)}

			{hasControls && (
				<ListToolbarControls className="gap-x-1 sm:gap-x-3 flex shrink-0 items-center">
					{/* No room on a phone, where grids already fit the screen (as in smart lists) */}
					{sizeControls && <div className="md:flex hidden items-center">{sizeControls}</div>}

					<div className="gap-x-1 flex items-center">
						{orderControls}
						{filterControls}
					</div>

					{layoutControls}
				</ListToolbarControls>
			)}
		</ListToolbar>
	)
}
