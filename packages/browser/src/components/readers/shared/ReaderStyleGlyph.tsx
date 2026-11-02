import { cn } from '@stump/components'

type Props = {
	className?: string
}

/**
 * The reading settings glyph: a serif capital A with a smaller, two-storey lowercase a,
 * drawn on lucide's 24px grid with a matching round stroke, so it sits with the bar's other
 * icons. Bookish rather than technical (lucide's `ALargeSmall` is a geometric sans), which is
 * what a reader's type and page settings are about.
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
			<g transform="translate(0.7 0)">
				{/* A: the two strokes, the bar and its two feet */}
				<path d="M2.6 19 7.5 4.5 12.4 19" />
				<path d="M4.4 14h6.2" />
				<path d="M1.4 19h2.5M11.1 19h2.6" />
				{/* a: the hooked stem with its little tail, then the bowl */}
				<path d="M15.8 12.3C16.4 11.4 17.3 11 18.3 11 19.6 11 20.2 11.7 20.2 13V18.1c0 .6.35.95 1 .95" />
				<path d="M20.2 15C18.4 15 15.4 15.3 15.4 17.2 15.4 18.4 16.3 19.1 17.4 19.1 18.8 19.1 19.8 18.2 20.2 17.1" />
			</g>
		</svg>
	)
}
