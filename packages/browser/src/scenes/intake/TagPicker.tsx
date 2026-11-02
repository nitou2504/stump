import { useGraphQL } from '@stump/client'
import { Button, cn, Command, Popover, ToolTip } from '@stump/components'
import { Check, Plus } from 'lucide-react'
import { useMemo, useState } from 'react'
import { useDebouncedValue } from 'rooks'

import { intakeVocabularyQuery } from './queries'
import { confidenceLevel, type GenreChip, tagOrigin, toggleGenre } from './reviewState'

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
 * is; hovering tells why), plus a search over the tags the library already uses.
 */
export default function TagPicker({ libraryId, chips, chosen, onChange, disabled }: Props) {
	const genres = chips.filter((c) => c.kind !== 'FACET')
	const facets = chips.filter((c) => c.kind === 'FACET')
	const hasGuesses = chips.some((c) => tagOrigin(c) === 'guessed')

	return (
		<div className="gap-2 flex flex-col">
			{hasGuesses && (
				<p className="gap-3 text-xs flex flex-wrap text-muted-foreground">
					<span className="gap-1.5 inline-flex items-center">
						<span className="size-2 rounded-full bg-muted-foreground" aria-hidden /> from the book
					</span>
					<span className="gap-1.5 inline-flex items-center">
						<span className="size-2 rounded-full border border-muted-foreground" aria-hidden />
						guessed from similar books
					</span>
				</p>
			)}
			<ChipRow chips={genres} chosen={chosen} onChange={onChange} disabled={disabled} />
			{facets.length > 0 && (
				<ChipRow chips={facets} chosen={chosen} onChange={onChange} disabled={disabled} muted />
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

function ChipRow({
	chips,
	chosen,
	onChange,
	disabled,
	muted,
}: Omit<Props, 'libraryId'> & { muted?: boolean }) {
	if (!chips.length) return null
	return (
		<div className="gap-1.5 flex flex-wrap" role="group" aria-label={muted ? 'Facets' : 'Genres'}>
			{chips.map((chip) => {
				const isOn = chosen.includes(chip.value)
				const level = confidenceLevel(chip.confidence)
				const guessed = tagOrigin(chip) === 'guessed'
				const why = [
					chip.suggested ? (guessed ? 'Guessed from similar books' : 'From the book') : undefined,
					...chip.reasons,
					chip.libraryCount ? `${chip.libraryCount} books in the library have it` : undefined,
					chip.suggested ? `${Math.round(chip.confidence * 100)}% sure` : undefined,
				].filter(Boolean)
				return (
					<ToolTip key={chip.value} content={why.join(' · ')} isDisabled={!why.length} size="xs">
						<button
							type="button"
							disabled={disabled}
							aria-pressed={isOn}
							onClick={() => onChange(toggleGenre(chosen, chip.value))}
							className={cn(
								'gap-1.5 px-2.5 py-1 text-xs min-h-7 inline-flex max-w-full items-center rounded-full border transition-colors disabled:opacity-50',
								isOn
									? 'border-primary/40 bg-primary/15 text-foreground'
									: 'border-dashed border-border text-muted-foreground hover:border-foreground/40 hover:text-foreground',
								muted && !isOn && 'opacity-80',
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
			})}
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
