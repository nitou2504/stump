import { cn } from '@stump/components'
import { ArrowRight, X } from 'lucide-react'
import { useEffect } from 'react'

import { useTranslate } from '@/hooks/useTranslate'

export type OpenNote = {
	/** The popup's title: `Note [1]`, or the link's text for a whole page */
	title: string
	/** Sanitised note HTML (see `extractNote`) */
	html: string
}

type Props = {
	note: OpenNote
	onClose: () => void
	/** Go to the note's place in the book. Absent when the note's page can't be opened */
	onGoToNote?: () => void
}

/**
 * A book's footnote in a panel at the bottom of the reader, like KOReader's: the reader
 * stays where it is. Closed with its button, Escape, a tap on the page or turning the
 * page. It doesn't take the focus, so the arrow keys keep turning pages.
 */
export default function NotePopup({ note, onClose, onGoToNote }: Props) {
	const { translate } = useTranslate()

	useEffect(() => {
		const handleKeyDown = (event: KeyboardEvent) => {
			if (event.key !== 'Escape' || event.defaultPrevented) return
			event.preventDefault()
			onClose()
		}
		window.addEventListener('keydown', handleKeyDown, { capture: true })
		return () => window.removeEventListener('keydown', handleKeyDown, { capture: true })
	}, [onClose])

	const title = note.title || translate('shared.epubReader.links.note')

	return (
		<div
			// Above the reader's footer (33px), which would otherwise cover the note's last line
			className="inset-x-0 p-2 md:p-4 pointer-events-none fixed bottom-[33px] z-50 flex justify-center"
			data-testid="epub-note-popup"
		>
			<section
				role="dialog"
				aria-modal="false"
				aria-label={title}
				className="max-w-xl shadow-xl pointer-events-auto w-full rounded-xl border border-border bg-background text-foreground"
			>
				<header className="gap-1 pl-4 pr-1 pt-1 flex items-center">
					<h2 className="text-xs font-medium tracking-wide min-w-0 flex-1 truncate uppercase opacity-70">
						{title}
					</h2>
					{onGoToNote && (
						<button
							type="button"
							onClick={onGoToNote}
							className="h-9 gap-1 px-2.5 text-xs font-medium flex shrink-0 items-center rounded-md hover:bg-muted/80"
						>
							{translate('shared.epubReader.links.goToNote')}
							<ArrowRight className="h-3.5 w-3.5" />
						</button>
					)}
					<button
						type="button"
						onClick={onClose}
						aria-label={translate('shared.epubReader.links.close')}
						title={translate('shared.epubReader.links.close')}
						className="h-9 w-9 flex shrink-0 items-center justify-center rounded-md hover:bg-muted/80"
					>
						<X className="h-4 w-4" />
					</button>
				</header>
				<div
					className={cn(
						'px-4 pb-4 pt-1 text-sm leading-relaxed max-h-[40vh] overflow-y-auto overscroll-contain',
						'[&_li]:ml-4 [&_p]:my-1 [&_img]:h-auto [&_img]:max-w-full [&_ol]:list-decimal [&_ul]:list-disc',
					)}
					// The note is copied from the book into plain, safe HTML (see `extractNote`)
					dangerouslySetInnerHTML={{ __html: note.html }}
				/>
			</section>
		</div>
	)
}
