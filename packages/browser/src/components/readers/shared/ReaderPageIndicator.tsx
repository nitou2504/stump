import { cn } from '@stump/components'

import { useReaderStore } from '@/stores'

type Props = {
	/** e.g. `23 / 327`, `22–23 / 327` */
	text: string
	/** Desktop: clicking it opens "go to page" (taps on phones go to the page underneath) */
	onClick?: () => void
	title?: string
	className?: string
}

/**
 * The small page number in the bottom corner, the only reading progress shown over the
 * pages. It can be turned off in the reader settings.
 */
export default function ReaderPageIndicator({ text, onClick, title, className }: Props) {
	const shown = useReaderStore((state) => state.settings.showPageIndicator ?? true)
	if (!shown || !text) return null

	const classes = cn(
		'px-1.5 py-0.5 text-[11px] absolute z-30 rounded tabular-nums leading-none',
		'text-foreground/55 select-none',
		className,
	)
	// Clear of rounded corners and cutouts (the right one in landscape)
	const style = {
		bottom: 'calc(0.375rem + env(safe-area-inset-bottom))',
		right: 'calc(0.75rem + env(safe-area-inset-right))',
	}

	if (onClick) {
		return (
			<button
				type="button"
				onClick={onClick}
				title={title}
				className={cn(classes, 'hover:bg-foreground/10 hover:text-foreground pointer-coarse:pointer-events-none')}
				style={style}
				data-testid="reader-page-indicator"
			>
				{text}
			</button>
		)
	}

	return (
		<div
			aria-live="off"
			className={cn(classes, 'pointer-events-none')}
			style={style}
			data-testid="reader-page-indicator"
		>
			{text}
		</div>
	)
}
