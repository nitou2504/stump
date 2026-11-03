import { useGraphQL, useGraphQLMutation } from '@stump/client'
import { Button, cn } from '@stump/components'
import { MediaMetadataEditsQuery } from '@stump/graphql'
import { useQueryClient } from '@tanstack/react-query'
import { ChevronDown, History, Undo2 } from 'lucide-react'
import { useId, useState } from 'react'
import { toast } from 'sonner'

import { historyLabel, isListField, listDiff, saveDestination } from './history'
import { notifySaved } from './outcome'
import { mediaMetadataEditsQuery, revertMediaMetadataEditMutation } from './queries'

type Save = MediaMetadataEditsQuery['mediaMetadataEdits'][number]
type Change = Save['changes'][number]

type Props = {
	mediaId: string
	/** Reverting is held while the form has unsaved changes */
	disabled?: boolean
}

const WHEN = new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'short' })

const DESTINATION = {
	both: 'file and Stump',
	file: 'in the file',
	stump: 'in Stump only',
}

/**
 * The book's latest saves (who, when, what changed and where it went), each with a Revert
 * that puts the old values back through the same save path. Loaded when opened.
 */
export default function EditHistory({ mediaId, disabled }: Props) {
	const id = useId()
	const [open, setOpen] = useState(false)
	const client = useQueryClient()
	const { data, isLoading, isError } = useGraphQL(
		mediaMetadataEditsQuery,
		['mediaMetadataEdits', mediaId],
		{ limit: 10, mediaId },
		{ enabled: open },
	)
	const saves = data?.mediaMetadataEdits ?? []

	const {
		mutate: revert,
		isPending,
		variables,
	} = useGraphQLMutation(revertMediaMetadataEditMutation, {
		onError: (error) =>
			toast.error('The edit could not be reverted', {
				description: error instanceof Error ? error.message : undefined,
			}),
		onSuccess: ({ revertMediaMetadataEdit: { file, skippedFields } }) => {
			notifySaved(file, { reverted: true, skipped: skippedFields })
			client.invalidateQueries()
		},
	})

	return (
		<section className="gap-2 pt-3 flex flex-col border-t border-border">
			<button
				type="button"
				aria-expanded={open}
				aria-controls={`${id}-list`}
				onClick={() => setOpen((o) => !o)}
				className="gap-1.5 text-sm pointer-coarse:min-h-10 -ml-1 px-1 inline-flex items-center self-start rounded-md text-muted-foreground hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
			>
				<History className="size-4" aria-hidden />
				Edit history
				<ChevronDown
					className={cn(
						'size-4 transition-transform motion-reduce:transition-none',
						open && 'rotate-180',
					)}
					aria-hidden
				/>
			</button>

			{open && (
				<div id={`${id}-list`} className="gap-2 flex flex-col">
					{isLoading && <p className="text-xs text-muted-foreground">Loading…</p>}
					{isError && <p className="text-xs text-destructive">The history could not be loaded.</p>}
					{!isLoading && !isError && !saves.length && (
						<p className="text-xs text-muted-foreground">
							No edits yet. Every save is listed here and can be reverted.
						</p>
					)}
					{disabled && saves.length > 0 && (
						<p className="text-xs text-muted-foreground">
							Save or cancel your changes to revert an edit.
						</p>
					)}
					<ol className="gap-2 flex flex-col">
						{saves.map((save) => (
							<SaveEntry
								key={save.saveId}
								save={save}
								disabled={disabled || isPending}
								isReverting={isPending && variables?.saveId === save.saveId}
								onRevert={() => revert({ mediaId, saveId: save.saveId })}
							/>
						))}
					</ol>
				</div>
			)}
		</section>
	)
}

function SaveEntry({
	save,
	disabled,
	isReverting,
	onRevert,
}: {
	save: Save
	disabled?: boolean
	isReverting: boolean
	onRevert: () => void
}) {
	const when = WHEN.format(new Date(save.editedAt))
	return (
		<li
			className={cn(
				'gap-2 p-2.5 flex flex-col rounded-lg border border-border',
				save.revertedBy && 'bg-muted/40',
			)}
		>
			<div className="gap-2 flex items-start">
				<p className="text-xs min-w-0 flex-1 text-muted-foreground">
					<span className="text-foreground">{save.revertsSaveId ? 'Revert' : 'Edit'}</span>
					{' · '}
					<time dateTime={save.editedAt}>{when}</time>
					{save.editedBy && ` · ${save.editedBy}`}
					{' · '}
					{DESTINATION[saveDestination(save.changes)]}
				</p>
				{save.revertedBy ? (
					<span className="text-xs shrink-0 text-muted-foreground">Reverted</span>
				) : (
					<Button
						type="button"
						variant="outline"
						size="xs"
						className="shrink-0"
						onClick={onRevert}
						disabled={disabled}
						isLoading={isReverting}
					>
						<Undo2 /> Revert
					</Button>
				)}
			</div>
			<ul className="gap-1 flex flex-col">
				{save.changes.map((change) => (
					<ChangeLine key={change.field} change={change} />
				))}
			</ul>
		</li>
	)
}

function ChangeLine({ change }: { change: Change }) {
	const label = historyLabel(change.field)
	if (isListField(change.field)) {
		const { added, removed } = listDiff(change.oldValue, change.newValue)
		return (
			<li className="gap-1 text-xs flex flex-wrap items-baseline">
				<span className="mr-0.5 text-muted-foreground">{label}:</span>
				{removed.map((item) => (
					<span
						key={`-${item}`}
						className="px-1.5 rounded bg-destructive/10 text-muted-foreground line-through decoration-destructive/60"
					>
						<span className="sr-only">removed </span>
						{item}
					</span>
				))}
				{added.map((item) => (
					<span key={`+${item}`} className="px-1.5 rounded bg-primary/10 text-foreground">
						<span className="sr-only">added </span>+ {item}
					</span>
				))}
				{!added.length && !removed.length && (
					<span className="text-muted-foreground">reordered</span>
				)}
			</li>
		)
	}
	return (
		<li className="gap-1 text-xs min-w-0 flex items-baseline">
			<span className="shrink-0 text-muted-foreground">{label}:</span>
			<span className="min-w-0 line-clamp-2 break-words">
				<span className="text-muted-foreground line-through decoration-destructive/60">
					{change.oldValue || 'none'}
				</span>
				{' → '}
				<span className="text-foreground">{change.newValue || 'none'}</span>
			</span>
		</li>
	)
}
