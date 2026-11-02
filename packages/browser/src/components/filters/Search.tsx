import { cn, ProgressSpinner, usePreviousIsDifferent } from '@stump/components'
import { SearchIcon } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { useDebouncedValue } from 'rooks'

import { isEditableTarget } from '@/components/readers/annotations/shortcuts'

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
}

/**
 * A search input that debounces the onChange function
 */
export default function Search({
	initialValue,
	placeholder,
	onChange,
	isLoading,
	isDisabled,
	className,
}: Props) {
	const inputRef = useRef<HTMLInputElement>(null)
	const containerRef = useRef<HTMLDivElement>(null)
	const [expanded, setExpanded] = useState(() => !!initialValue)
	const [value, setValue] = useState(initialValue ?? '')
	const [debouncedValue] = useDebouncedValue(value, 500)

	const shouldCall = usePreviousIsDifferent(debouncedValue)

	useEffect(() => {
		if (initialValue) {
			setValue(initialValue)
			setExpanded(true)
		}
	}, [initialValue])

	useEffect(() => {
		if (debouncedValue !== undefined && shouldCall) {
			onChange(debouncedValue)
		}
	}, [debouncedValue, onChange, shouldCall])

	const handleExpand = () => {
		if (isDisabled) return
		setExpanded(true)
	}

	useEffect(() => {
		if (!expanded) return
		const el = containerRef.current
		if (!el) return

		const onEnd = (e: TransitionEvent) => {
			if (e.propertyName === 'width') inputRef.current?.focus()
		}
		el.addEventListener('transitionend', onEnd)
		return () => el.removeEventListener('transitionend', onEnd)
	}, [expanded])

	// `/` opens the search (as in many apps), and Firefox's quick find bar doesn't open
	useEffect(() => {
		if (isDisabled) return undefined
		const onKeyDown = (event: KeyboardEvent) => {
			if (event.key !== '/' || event.defaultPrevented) return
			if (event.ctrlKey || event.metaKey || event.altKey) return
			if (isEditableTarget(event.target)) return
			if (document.querySelector('[role="dialog"][data-state="open"]')) return
			event.preventDefault()
			if (expanded) {
				inputRef.current?.focus()
			} else {
				// focused once it has grown (see the transition above)
				setExpanded(true)
			}
		}
		window.addEventListener('keydown', onKeyDown)
		return () => window.removeEventListener('keydown', onKeyDown)
	}, [isDisabled, expanded])

	const handleBlur = () => {
		if (!value) setExpanded(false)
	}

	const showLoader = isLoading && !!value

	return (
		<div
			ref={containerRef}
			role={expanded ? undefined : 'button'}
			tabIndex={expanded ? undefined : 0}
			onClick={expanded ? undefined : handleExpand}
			onKeyDown={expanded ? undefined : (e) => e.key === 'Enter' && handleExpand()}
			title={isDisabled ? "This functionality isn't available right now" : undefined}
			className={cn(
				'h-8 pointer-coarse:h-10 gap-2 text-sm ease-in-out relative flex cursor-pointer items-center overflow-hidden rounded-md border border-border bg-transparent transition-all duration-300',
				'text-muted-foreground hover:bg-muted/30 hover:text-foreground',
				'disabled:cursor-not-allowed disabled:opacity-50',
				// expanded, it may shrink: on a phone its full width pushed the controls next to
				// it (sort, filters, layout) off-screen
				expanded ? 'sm:w-2/5 min-w-24 w-full shrink cursor-text' : 'w-32 shrink-0',
				className,
			)}
		>
			<SearchIcon className="ml-2.5 h-4 w-4 shrink-0" />

			{expanded ? (
				<input
					ref={inputRef}
					type="text"
					value={value}
					onChange={(e) => setValue(e.target.value)}
					onBlur={handleBlur}
					placeholder={placeholder || 'Search'}
					disabled={isDisabled}
					className="pr-8 text-sm h-full w-full bg-transparent text-foreground outline-none placeholder:text-muted-foreground"
				/>
			) : (
				<span className="pr-2.5 text-sm whitespace-nowrap select-none">Search</span>
			)}

			{showLoader && (
				<div className="inset-y-0 right-0 pr-2.5 absolute flex items-center">
					<ProgressSpinner size="sm" />
				</div>
			)}
		</div>
	)
}
