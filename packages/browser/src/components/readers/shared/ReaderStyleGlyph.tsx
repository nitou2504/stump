import { cn } from '@stump/components'

type Props = {
	className?: string
}

/**
 * The reading settings glyph, `Aa`: a monoline capital A and a single-storey a, drawn on
 * lucide's 24px grid with its round 1.75 stroke so it sits with the bar's other icons and
 * stays clear at 16px (see the reader design's components board)
 */
export default function ReaderStyleGlyph({ className }: Props) {
	return (
		<svg
			aria-hidden
			viewBox="0 0 24 24"
			fill="none"
			stroke="currentColor"
			strokeWidth="1.75"
			strokeLinecap="round"
			strokeLinejoin="round"
			className={cn('h-[18px] w-[18px] shrink-0', className)}
			data-testid="reader-style-glyph"
		>
			{/* A: the two strokes and the bar */}
			<path d="M2.75 19.25 8 5l5.25 14.25" />
			<path d="M4.75 14.25h6.5" />
			{/* a: the bowl and its stem */}
			<circle cx="17.6" cy="16.15" r="3.1" />
			<path d="M20.7 12.9v6.35" />
		</svg>
	)
}
