import { Undo2, X } from 'lucide-react'

import { useTranslate } from '@/hooks/useTranslate'

type Props = {
	onReturn: () => void
	onHide: () => void
}

/**
 * Shown after following a link inside the book: back to where the link was (the browser's
 * Back does the same)
 */
export default function ReturnChip({ onReturn, onHide }: Props) {
	const { translate } = useTranslate()

	return (
		<div
			className="bottom-16 shadow-lg fixed left-1/2 z-40 flex -translate-x-1/2 items-center rounded-full border border-border bg-background text-foreground"
			data-testid="epub-link-return"
		>
			<button
				type="button"
				onClick={onReturn}
				title={translate('shared.epubReader.links.returnTitle')}
				className="h-9 gap-1.5 pl-3 pr-2 text-sm font-medium flex items-center rounded-l-full hover:bg-muted/80"
			>
				<Undo2 className="h-4 w-4" />
				{translate('shared.epubReader.links.return')}
			</button>
			<span aria-hidden className="h-5 w-px bg-border" />
			<button
				type="button"
				onClick={onHide}
				aria-label={translate('shared.epubReader.links.hideReturn')}
				title={translate('shared.epubReader.links.hideReturn')}
				className="h-9 w-9 flex items-center justify-center rounded-r-full hover:bg-muted/80"
			>
				<X className="h-3.5 w-3.5" />
			</button>
		</div>
	)
}
