import { cn } from '@stump/components'
import { type ComponentProps, createContext, useContext, useMemo, useState } from 'react'

type ListToolbarContextValue = {
	/**
	 * Whether the search is expanded across the toolbar. On phones the other controls step
	 * aside while it is (see `ListToolbarControls`)
	 */
	isSearchOpen: boolean
	setSearchOpen: (isOpen: boolean) => void
	/**
	 * Whether the toolbar has controls next to the search. Only then does the search fold
	 * into an icon on phones: alone in its toolbar (the home page), it keeps the whole row
	 */
	hasControls: boolean
}

const ListToolbarContext = createContext<ListToolbarContextValue | null>(null)

/** The toolbar a search sits in, if any */
export const useListToolbar = () => useContext(ListToolbarContext)

type ListToolbarProps = ComponentProps<'header'> & {
	hasControls?: boolean
}

/**
 * The row above a list: the search, then the list's controls (sort, filters, grouping,
 * layout). On phones it stays one row: the search is an icon that opens across the row,
 * and the controls step aside while it is open.
 */
export function ListToolbar({ hasControls = true, children, ...props }: ListToolbarProps) {
	const [isSearchOpen, setSearchOpen] = useState(false)
	const value = useMemo(
		() => ({ isSearchOpen, setSearchOpen, hasControls }),
		[isSearchOpen, hasControls],
	)

	return (
		<ListToolbarContext.Provider value={value}>
			<header data-list-toolbar="" data-search-open={isSearchOpen || undefined} {...props}>
				{children}
			</header>
		</ListToolbarContext.Provider>
	)
}

/** The list's controls next to the search, hidden on phones while the search is open */
export function ListToolbarControls({ className, ...props }: ComponentProps<'div'>) {
	const toolbar = useListToolbar()
	return (
		<div
			data-list-toolbar-controls=""
			className={cn(toolbar?.isSearchOpen && 'max-sm:hidden', className)}
			{...props}
		/>
	)
}
