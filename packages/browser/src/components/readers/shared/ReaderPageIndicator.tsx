import { cn } from '@stump/components'

import { useReaderStore } from '@/stores'

type Props = {
	/** e.g. `23 / 327`, `22–23 / 327` */
	text: string
	/** Opens "go to": a page number (PDF), the contents (EPUB) or the pages (comics) */
	onClick?: () => void
	title?: string
	className?: string
}

/**
 * The small page number in the bottom corner, the only reading progress shown over the
 * pages. It can be turned off in the reader settings.
 *
 * It's a tap target of its own on every device: a tap on it opens "go to" and never reaches
 * the page under it (where the right edge would turn the page). It stays visually small, with
 * a comfortable, invisible hit area around it.
 */
export default function ReaderPageIndicator({ text, onClick, title, className }: Props) {
	const shown = useReaderStore((state) => state.settings.showPageIndicator ?? true)
	if (!shown || !text) return null

	const pill = 'px-1.5 py-0.5 rounded text-[11px] leading-none tabular-nums text-foreground/55'
	// Clear of rounded corners and cutouts (the right one in landscape)
	const corner = {
		bottom: 'env(safe-area-inset-bottom)',
		right: 'env(safe-area-inset-right)',
	}

	if (onClick) {
		const stop = (event: React.SyntheticEvent) => event.stopPropagation()
		return (
			<button
				type="button"
				onClick={(event) => {
					event.stopPropagation()
					onClick()
				}}
				onPointerDown={stop}
				onPointerUp={stop}
				onTouchStart={stop}
				onTouchEnd={stop}
				title={title}
				aria-label={title ? `${text}, ${title}` : text}
				// the hit area: at least 40px each way, the number in its corner as before
				className={cn(
					'group pr-3 pb-1.5 pl-3 pt-3 min-h-11 min-w-16 absolute z-51 flex items-end justify-end select-none',
					className,
				)}
				style={corner}
				data-testid="reader-page-indicator"
			>
				<span
					className={cn(
						pill,
						'group-hover:bg-foreground/10 group-hover:text-foreground group-active:bg-foreground/10',
					)}
				>
					{text}
				</span>
			</button>
		)
	}

	return (
		<div
			aria-live="off"
			className={cn('pr-3 pb-1.5 pointer-events-none absolute z-30 select-none', className)}
			style={corner}
			data-testid="reader-page-indicator"
		>
			<span className={pill}>{text}</span>
		</div>
	)
}
