import { cn } from '@stump/components'
import { Redo2, Undo2 } from 'lucide-react'

import { useTranslate } from '@/hooks/useTranslate'

import type { NavEntry } from './navHistory'

type Props = {
	back: NavEntry<unknown> | null
	forward: NavEntry<unknown> | null
	onBack: () => void
	onForward: () => void
	className?: string
}

/**
 * At the top of the contents (EPUB, PDF) and of a comic's pages: back to where the reader
 * was before its last jump, e.g. "Back to Page 23 · Chapter 2", and forward again after
 * going back. Nothing shows until the reader has jumped somewhere.
 */
export default function NavHistoryRow({ back, forward, onBack, onForward, className }: Props) {
	const { translate } = useTranslate()
	if (!back && !forward) return null

	const backText = back ? translate('shared.reader.history.back', { place: back.label }) : ''
	const forwardText = forward
		? translate('shared.reader.history.forward', { place: forward.label })
		: ''

	return (
		// min-w-0: in a dialog's grid a long place name is cut short instead of widening it
		<div
			className={cn('gap-1 min-w-0 flex items-center', className)}
			data-testid="reader-nav-history"
		>
			{back && (
				<button
					type="button"
					onClick={onBack}
					title={backText}
					className="h-9 gap-2 px-2 text-sm min-w-0 flex flex-1 items-center rounded-md text-left text-foreground/90 hover:bg-muted focus-visible:ring-2 focus-visible:ring-primary focus-visible:outline-none"
					data-testid="reader-nav-back"
				>
					<Undo2 aria-hidden className="h-4 w-4 shrink-0 text-muted-foreground" />
					<span className="truncate">{backText}</span>
				</button>
			)}
			{forward && (
				<button
					type="button"
					onClick={onForward}
					title={forwardText}
					aria-label={forwardText}
					className={cn(
						'h-9 gap-2 px-2 text-sm min-w-0 flex items-center rounded-md text-foreground/90 hover:bg-muted focus-visible:ring-2 focus-visible:ring-primary focus-visible:outline-none',
						back ? 'max-w-[45%] shrink-0' : 'flex-1',
					)}
					data-testid="reader-nav-forward"
				>
					{/* beside "Back to", only the arrow and where it leads, cut short */}
					<span className={cn('truncate', back && 'text-muted-foreground')}>
						{back ? forward.label : forwardText}
					</span>
					<Redo2 aria-hidden className="h-4 w-4 shrink-0 text-muted-foreground" />
				</button>
			)}
		</div>
	)
}
