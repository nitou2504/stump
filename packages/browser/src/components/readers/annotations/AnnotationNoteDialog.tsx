import { Button, cn, Dialog, Text, TextArea } from '@stump/components'
import type { AnnotationStyle } from '@stump/graphql'
import { useEffect, useState } from 'react'

import { useTranslate } from '@/hooks/useTranslate'

import { ANNOTATION_STYLES } from './styles'

type Props = {
	open: boolean
	mode: 'create' | 'edit'
	quotedText?: string | null
	initialNote?: string | null
	initialStyle: AnnotationStyle
	isPending?: boolean
	onOpenChange: (open: boolean) => void
	onSave: (note: string, style: AnnotationStyle) => void
	onDelete?: () => void
}

/**
 * Writes or edits an annotation's note: the quoted text, the style it is drawn with, and the
 * note. Ctrl/Cmd+Enter saves.
 */
export default function AnnotationNoteDialog({
	open,
	mode,
	quotedText,
	initialNote,
	initialStyle,
	isPending,
	onOpenChange,
	onSave,
	onDelete,
}: Props) {
	const { translate } = useTranslate()
	const [note, setNote] = useState(initialNote ?? '')
	const [style, setStyle] = useState(initialStyle)

	useEffect(() => {
		if (open) {
			setNote(initialNote ?? '')
			setStyle(initialStyle)
		}
	}, [open, initialNote, initialStyle])

	const save = () => onSave(note.trim(), style)

	return (
		<Dialog open={open} onOpenChange={isPending ? undefined : onOpenChange}>
			<Dialog.Content size="sm">
				<Dialog.Header>
					<Dialog.Title>
						{translate(
							mode === 'create'
								? 'shared.readerAnnotations.addNote'
								: 'shared.readerAnnotations.editNote',
						)}
					</Dialog.Title>
					<Dialog.Close onClick={() => onOpenChange(false)} />
				</Dialog.Header>

				{quotedText && (
					<Text
						variant="muted"
						size="sm"
						className="pl-3 line-clamp-4 border-l-2 border-border italic"
					>
						&ldquo;{quotedText}&rdquo;
					</Text>
				)}

				<div
					role="radiogroup"
					aria-label={translate('shared.readerAnnotations.style')}
					className="gap-1 flex items-center"
				>
					{ANNOTATION_STYLES.map((meta) => {
						const label = translate(meta.labelKey)
						const checked = meta.style === style
						return (
							<button
								key={meta.style}
								type="button"
								role="radio"
								aria-checked={checked}
								aria-label={label}
								title={label}
								onClick={() => setStyle(meta.style)}
								className={cn(
									'size-9 flex items-center justify-center rounded-md border border-transparent hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none',
									{ 'border-border bg-muted': checked },
								)}
							>
								<meta.icon className="h-4 w-4" />
							</button>
						)
					})}
				</div>

				<TextArea
					label={translate('shared.epubReader.annotation.note')}
					placeholder={translate('shared.epubReader.annotation.optionalNote')}
					value={note}
					onChange={(event) => setNote(event.target.value)}
					onKeyDown={(event) => {
						if (event.key === 'Enter' && (event.metaKey || event.ctrlKey)) {
							event.preventDefault()
							save()
						}
					}}
					rows={4}
					autoFocus
				/>

				<Dialog.Footer>
					{mode === 'edit' && onDelete && (
						<Button
							variant="destructive"
							size="sm"
							disabled={isPending}
							onClick={onDelete}
							className="sm:mr-auto"
						>
							{translate('shared.common.delete')}
						</Button>
					)}
					<Button
						variant="outline"
						size="sm"
						disabled={isPending}
						onClick={() => onOpenChange(false)}
					>
						{translate('shared.common.cancel')}
					</Button>
					<Button size="sm" disabled={isPending} onClick={save}>
						{translate('shared.common.save')}
					</Button>
				</Dialog.Footer>
			</Dialog.Content>
		</Dialog>
	)
}
