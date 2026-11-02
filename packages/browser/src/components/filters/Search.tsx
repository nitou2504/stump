import { cn, IconButton, ProgressSpinner, usePreviousIsDifferent } from '@stump/components'
import { SearchIcon, X } from 'lucide-react'
import { type Ref, useCallback, useEffect, useImperativeHandle, useRef, useState } from 'react'
import { useDebouncedValue, useMediaMatch } from 'rooks'

import { useListToolbar } from './ListToolbar'
import { registerListSearch } from './searchRegistry'

/** What a parent can do with a search through its `ref` */
export type SearchHandle = {
	/** Opens the search (expanding it on phones) and focuses its input */
	open: () => void
}

type Props = {
	/**
	 * The initial value of the search input. This is useful for when you load onto
	 * a page with a search already in the URL.
	 */
	initialValue?: string
	/**
	 * The placeholder text to display in the search input.
	 */
	placeholder?: string
	/**
	 * The function to call when the search input changes. This is debounced by 500ms.
	 */
	onChange: (value: string) => void
	/**
	 * Whether or not the search input should display a loading indicator.
	 */
	isLoading?: boolean
	/**
	 *  Whether or not the search input should be disabled
	 */
	isDisabled?: boolean
	className?: string
	ref?: Ref<SearchHandle>
}

/** Below this width (Tailwind's `sm`) a search next to other controls folds into an icon */
const NARROW_QUERY = '(max-width: 639px)'

/**
 * A search input that debounces the onChange function.
 *
 * On wider screens it is a compact input that widens while in use. On phones, in a toolbar
 * with other controls, it is an icon button (marked with a dot while a search is active)
 * that opens across the toolbar's row; the controls step aside until it closes again (on
 * blur, Enter or Escape; the X clears the text, or closes it once empty). Pages can open it
 * through its `ref` or, for a page-wide shortcut, `openListSearch()` (see `searchRegistry`).
 */
export default function Search({
	initialValue,
	placeholder,
	onChange,
	isLoading,
	isDisabled,
	className,
	ref,
}: Props) {
	const inputRef = useRef<HTMLInputElement>(null)
	const containerRef = useRef<HTMLDivElement>(null)
	const [value, setValue] = useState(initialValue ?? '')
	const [debouncedValue] = useDebouncedValue(value, 500)
	const shouldCall = usePreviousIsDifferent(debouncedValue)

	const toolbar = useListToolbar()
	const isNarrow = useMediaMatch(NARROW_QUERY)
	const isCollapsible = isNarrow && !!toolbar?.hasControls
	const [isOpen, setIsOpen] = useState(false)
	const isCollapsed = isCollapsible && !isOpen

	useEffect(() => {
		if (initialValue) {
			setValue(initialValue)
		}
	}, [initialValue])

	useEffect(() => {
		if (debouncedValue !== undefined && shouldCall) {
			onChange(debouncedValue)
		}
	}, [debouncedValue, onChange, shouldCall])

	// The toolbar hides its other controls while the search takes the row
	const setToolbarSearchOpen = toolbar?.setSearchOpen
	useEffect(() => {
		setToolbarSearchOpen?.(isCollapsible && isOpen)
	}, [setToolbarSearchOpen, isCollapsible, isOpen])
	useEffect(() => () => setToolbarSearchOpen?.(false), [setToolbarSearchOpen])

	// The input stays in the page while folded (just out of sight), so opening the search
	// focuses it within the tap itself: phones only raise the keyboard for that
	const open = useCallback(() => {
		if (isDisabled) return
		inputRef.current?.focus()
		setIsOpen(true)
	}, [isDisabled])

	useImperativeHandle(ref, () => ({ open }), [open])

	useEffect(
		() =>
			registerListSearch({
				open,
				isVisible: () => !!containerRef.current?.offsetParent,
			}),
		[open],
	)

	const close = () => {
		setIsOpen(false)
		inputRef.current?.blur()
	}

	const handleBlur = (e: React.FocusEvent) => {
		// the clear button keeps the search open
		if (containerRef.current?.contains(e.relatedTarget as Node)) return
		setIsOpen(false)
	}

	const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
		if (e.key === 'Escape') {
			close()
		} else if (e.key === 'Enter' && isCollapsible) {
			// the phone keyboard's search key: show the results, and the controls again
			close()
		}
	}

	const handleClear = () => {
		if (value) {
			setValue('')
			inputRef.current?.focus()
		} else {
			close()
		}
	}

	const showLoader = isLoading && !!value
	const showClear = !isCollapsed && (!!value || isCollapsible)

	return (
		<div
			ref={containerRef}
			data-list-toolbar-search=""
			data-state={isCollapsed ? 'collapsed' : 'open'}
			title={isDisabled ? "This functionality isn't available right now" : undefined}
			className={cn(
				'relative flex items-center',
				isCollapsed
					? 'shrink-0'
					: isNarrow
						? 'min-w-0 flex-1'
						: cn(
								'w-48 ease-out focus-within:w-72 shrink transition-[width] duration-200',
								value && 'w-72',
							),
				className,
			)}
		>
			{isCollapsed && (
				<IconButton
					variant="ghost"
					size="sm"
					className="relative shrink-0 hover:bg-accent"
					onClick={open}
					disabled={isDisabled}
					aria-label={value ? `Search: ${value}` : 'Search'}
					data-active={value ? '' : undefined}
				>
					<SearchIcon className="h-4 w-4" />
					{value && (
						<span
							aria-hidden
							className="right-1.5 top-1.5 size-2 absolute rounded-full bg-primary ring-2 ring-background"
						/>
					)}
				</IconButton>
			)}

			<div
				className={cn(
					'h-8 pointer-coarse:h-10 gap-2 text-sm flex w-full items-center rounded-md border border-border bg-transparent text-muted-foreground transition-colors',
					'focus-within:border-ring focus-within:text-foreground hover:text-foreground',
					// folded: out of sight but still focusable, see `open`
					isCollapsed && 'size-0 pointer-events-none absolute overflow-hidden border-0 opacity-0',
				)}
			>
				<SearchIcon className="ml-2.5 h-4 w-4 shrink-0" aria-hidden />
				<input
					ref={inputRef}
					type="search"
					enterKeyHint="search"
					value={value}
					onChange={(e) => setValue(e.target.value)}
					onFocus={() => setIsOpen(true)}
					onBlur={handleBlur}
					onKeyDown={handleKeyDown}
					placeholder={placeholder || 'Search'}
					aria-label={placeholder || 'Search'}
					disabled={isDisabled}
					tabIndex={isCollapsed ? -1 : undefined}
					className={cn(
						'min-w-0 text-sm h-full flex-1 bg-transparent text-foreground outline-none placeholder:text-muted-foreground',
						// the browser's own clear button: this one has the X below
						'[&::-webkit-search-cancel-button]:appearance-none',
						!showClear && !showLoader && 'pr-2.5',
					)}
				/>

				{showLoader && <ProgressSpinner size="sm" className="shrink-0" />}

				{showClear && (
					<button
						type="button"
						// keep the focus in the input: a blur would close the search first
						onMouseDown={(e) => e.preventDefault()}
						onClick={handleClear}
						aria-label={value ? 'Clear search' : 'Close search'}
						className="mr-1 size-6 pointer-coarse:size-8 flex shrink-0 items-center justify-center rounded-sm text-muted-foreground hover:bg-accent hover:text-foreground"
					>
						<X className="h-3.5 w-3.5" />
					</button>
				)}
			</div>
		</div>
	)
}
