import { useGraphQL } from '@stump/client'
import { Button, cn, Command, Popover, ToolTip } from '@stump/components'
import { Check, Plus } from 'lucide-react'
import { useMemo, useState } from 'react'
import { useDebouncedValue } from 'rooks'

import { intakeVocabularyQuery } from './queries'
import { confidenceLevel, type GenreChip, tagWhy, toggleGenre } from './reviewState'

type Props = {
	libraryId: string
	chips: GenreChip[]
	/** The tags that will be written */
	chosen: string[]
	onChange: (genres: string[]) => void
	disabled?: boolean
}

const DOT = {
	high: 'bg-emerald-500 border-emerald-500',
	low: 'bg-amber-500 border-amber-500',
	medium: 'bg-sky-500 border-sky-500',
}

/**
 * The tags of a book: the server's suggestions as chips you toggle (a dot shows how sure it
 * is; a hollow, italic chip is a guess from similar books), why each proposed tag is there,
 * and a search over the tags the library already uses.
 */
export default function TagPicker({ libraryId, chips, chosen, onChange, disabled }: Props) {
	const genres = chips.filter((c) => c.kind !== 'FACET')
	const facets = chips.filter((c) => c.kind === 'FACET')
	// the "why" of every tag the server proposed and of everything that will be written
	const explained = chips.filter((c) => c.preselected || c.chosen)

	return (
		<div className="gap-2 flex flex-col">
			<ChipRow chips={genres} chosen={chosen} onChange={onChange} disabled={disabled} />
			{facets.length > 0 && (
				<ChipRow chips={facets} chosen={chosen} onChange={onChange} disabled={disabled} muted />
			)}
			{explained.length > 0 && (
				<ul
					className="gap-0.5 text-xs flex flex-col text-muted-foreground"
					aria-label="Why these tags"
				>
					{explained.map((chip) => (
						<li key={chip.value} className="gap-1.5 min-w-0 flex items-baseline">
							<span
								className={cn(
									'size-2 shrink-0 translate-y-px rounded-full border border-muted-foreground',
									chip.chosen && chip.origin !== 'guessed' && 'bg-muted-foreground',
									chip.chosen && chip.origin === 'guessed' && 'border-warning bg-warning/40',
								)}
								aria-hidden
							/>
							<span
								className={cn('shrink-0 text-foreground', chip.origin === 'guessed' && 'italic')}
							>
								{chip.value}
							</span>
							<span className="min-w-0 line-clamp-2 break-words" title={chip.reasons.join(' · ')}>
								{tagWhy(chip)}
								{chip.origin === 'guessed' &&
									!chip.chosen &&
									chip.preselected &&
									' — not ticked, tick it if right'}
							</span>
						</li>
					))}
				</ul>
			)}
			<div>
				<AddTag
					libraryId={libraryId}
					chosen={chosen}
					onAdd={(value) => onChange(toggleGenre(chosen, value))}
					disabled={disabled}
				/>
			</div>
		</div>
	)
}

/** A toggle chip for a tag; guessed ones are hollow and in italics */
export function TagChip({
	chip,
	isOn,
	onToggle,
	disabled,
	size = 'md',
}: {
	chip: GenreChip
	isOn: boolean
	onToggle: () => void
	disabled?: boolean
	size?: 'sm' | 'md'
}) {
	const level = confidenceLevel(chip.confidence)
	const guessed = chip.origin === 'guessed'
	const why = [
		chip.suggested ? (guessed ? 'Guessed from similar books' : 'From the book') : undefined,
		...chip.reasons,
		chip.libraryCount ? `${chip.libraryCount} books in the library have it` : undefined,
		chip.suggested ? `${Math.round(chip.confidence * 100)}% sure` : undefined,
	].filter(Boolean)
	return (
		<ToolTip content={why.join(' · ')} isDisabled={!why.length} size="xs">
			<button
				type="button"
				disabled={disabled}
				aria-pressed={isOn}
				onClick={onToggle}
				className={cn(
					'gap-1.5 inline-flex max-w-full items-center rounded-full border transition-colors disabled:opacity-50',
					size === 'sm' ? 'px-2 py-0.5 text-xs min-h-6' : 'px-2.5 py-1 text-xs min-h-7',
					isOn
						? guessed
							? 'border-warning/60 bg-warning/15 text-foreground'
							: 'border-primary/40 bg-primary/15 text-foreground'
						: 'border-dashed border-border text-muted-foreground hover:border-foreground/40 hover:text-foreground',
				)}
			>
				{chip.suggested && (
					<span
						className={cn(
							'size-2 shrink-0 rounded-full border',
							DOT[level],
							guessed && 'bg-transparent',
						)}
						aria-hidden
					/>
				)}
				<span className={cn('truncate', guessed && 'italic')}>{chip.value}</span>
				{guessed && <span className="sr-only">(guessed)</span>}
				{isOn && <Check className="h-3 w-3 shrink-0" aria-hidden />}
			</button>
		</ToolTip>
	)
}

function ChipRow({
	chips,
	chosen,
	onChange,
	disabled,
	muted,
}: Omit<Props, 'libraryId'> & { muted?: boolean }) {
	if (!chips.length) return null
	return (
		<div
			className={cn('gap-1.5 flex flex-wrap', muted && 'opacity-90')}
			role="group"
			aria-label={muted ? 'Facets' : 'Genres'}
		>
			{chips.map((chip) => (
				<TagChip
					key={chip.value}
					chip={chip}
					isOn={chosen.includes(chip.value)}
					disabled={disabled}
					onToggle={() => onChange(toggleGenre(chosen, chip.value))}
				/>
			))}
		</div>
	)
}

function AddTag({
	libraryId,
	chosen,
	onAdd,
	disabled,
}: {
	libraryId: string
	chosen: string[]
	onAdd: (value: string) => void
	disabled?: boolean
}) {
	const [open, setOpen] = useState(false)
	const [search, setSearch] = useState('')
	const [debounced] = useDebouncedValue(search, 250)

	const { data, isFetching } = useGraphQL(
		intakeVocabularyQuery,
		['intakeVocabulary', libraryId, debounced],
		{ libraryId, limit: 25, search: debounced || undefined },
		{ enabled: open },
	)
	const options = useMemo(
		() => (data?.intakeVocabulary ?? []).filter((v) => !chosen.includes(v.value)),
		[data, chosen],
	)

	return (
		<Popover open={open} onOpenChange={setOpen}>
			<Popover.Trigger asChild>
				<Button variant="ghost" size="xs" disabled={disabled} className="text-muted-foreground">
					<Plus /> Add a tag
				</Button>
			</Popover.Trigger>
			<Popover.Content
				align="start"
				className="p-0 w-[min(22rem,calc(100vw-2rem))]"
				collisionPadding={12}
			>
				<Command shouldFilter={false}>
					<Command.Input
						placeholder="Search the library's tags…"
						value={search}
						onValueChange={setSearch}
					/>
					<Command.List className="max-h-[min(18rem,50vh)]">
						{options.map((option) => (
							<Command.Item
								key={option.value}
								value={option.value}
								onSelect={() => {
									onAdd(option.value)
									setSearch('')
								}}
								className="gap-2"
							>
								<span className="flex-1 truncate">{option.value}</span>
								<span className="text-xs text-muted-foreground">{option.count}</span>
							</Command.Item>
						))}
						{!options.length && (
							<div className="py-6 text-sm text-center text-muted-foreground">
								{isFetching ? 'Searching…' : 'No matching tag in the library'}
							</div>
						)}
					</Command.List>
				</Command>
			</Popover.Content>
		</Popover>
	)
}
