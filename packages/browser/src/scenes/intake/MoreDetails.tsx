import { ChevronDown } from 'lucide-react'
import type { ReactNode } from 'react'

type Props = {
	children: ReactNode
	/** What is inside, in a few words, so nobody opens it to find out */
	hint?: string
}

/**
 * The part of a card most people never open: a native disclosure, so it works from the keyboard
 * and with a screen reader without any state of ours, and what is inside can't be tabbed to
 * while it is closed
 */
export default function MoreDetails({ children, hint }: Props) {
	return (
		<details className="group/more rounded-lg">
			<summary className="gap-1.5 px-1 py-1.5 pointer-coarse:py-3 text-sm font-medium flex w-fit cursor-pointer list-none items-center rounded-md text-muted-foreground select-none marker:hidden hover:text-foreground focus-visible:ring-[3px] focus-visible:ring-ring/50 focus-visible:outline-none [&::-webkit-details-marker]:hidden">
				<ChevronDown
					className="h-4 w-4 transition-transform group-open/more:rotate-180"
					aria-hidden
				/>
				<span className="whitespace-nowrap">More details</span>
				{hint && (
					<span className="text-xs font-normal sm:inline hidden text-muted-foreground/70">
						· {hint}
					</span>
				)}
			</summary>
			<div className="mt-2 gap-4 p-3 flex flex-col rounded-lg bg-muted/30">{children}</div>
		</details>
	)
}
