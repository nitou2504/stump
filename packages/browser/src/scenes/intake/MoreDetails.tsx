import { ChevronDown } from 'lucide-react'
import { type ReactNode, useEffect, useRef } from 'react'

type Props = {
	children: ReactNode
	/** What is inside, in a few words, so nobody opens it to find out */
	hint?: string
	/**
	 * Opens it when it turns true, for what has to be fixed in here (the folder, the volumes), and
	 * never closes it: whoever opened it, or closed it again, decides from there
	 */
	open?: boolean
}

/**
 * The part of a card most people never open: a native disclosure, so it works from the keyboard
 * and with a screen reader without any state of ours, and what is inside can't be tabbed to
 * while it is closed
 */
export default function MoreDetails({ children, hint, open }: Props) {
	const details = useRef<HTMLDetailsElement>(null)

	// only ever opens: with `open={problems > 0}` the fold would shut under somebody's hands the
	// moment the last problem is fixed
	useEffect(() => {
		if (open && details.current) details.current.open = true
	}, [open])

	return (
		<details ref={details} className="group/more rounded-lg">
			<summary className="gap-1.5 px-1 py-1.5 pointer-coarse:py-3 text-sm font-medium flex w-fit cursor-pointer list-none items-center rounded-md text-muted-foreground select-none marker:hidden hover:text-foreground focus-visible:ring-[3px] focus-visible:ring-ring/50 focus-visible:outline-none [&::-webkit-details-marker]:hidden">
				<ChevronDown
					className="h-4 w-4 transition-transform group-open/more:rotate-180 motion-reduce:transition-none"
					aria-hidden
				/>
				<span className="whitespace-nowrap">More details</span>
				{hint && (
					<span className="text-xs font-normal sm:inline hidden text-muted-foreground">
						· {hint}
					</span>
				)}
			</summary>
			<div className="mt-2 gap-4 p-3 flex flex-col rounded-lg bg-muted/30">{children}</div>
		</details>
	)
}
