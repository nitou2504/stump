import { useGraphQL } from '@stump/client'
import { Button, cn, Command, Popover } from '@stump/components'
import { MetadataFacetField } from '@stump/graphql'
import { AlertTriangle, Plus, X } from 'lucide-react'
import { useMemo, useState } from 'react'

import { metadataVocabularyQuery } from './queries'
import { creatableValue, groupVocabulary, matchesSearch, type TagGroup, tagGroup } from './tags'

type Props = {
	field: MetadataFacetField.Genres | MetadataFacetField.Writers
	libraryId?: string
	values: string[]
	onChange: (values: string[]) => void
	/** The text of the button that opens the picker, e.g. "Add a genre" */
	addLabel: string
	disabled?: boolean
}

const GROUP_LABEL: Record<TagGroup, string> = {
	facet: 'Facets: origin, era, level, audience…',
	genre: 'Genres',
	subgenre: 'Subgenres',
}

/** The library's values of a field, cached for the session's editing */
export function useVocabulary(field: MetadataFacetField, libraryId?: string, enabled = true) {
	const { data, isLoading } = useGraphQL(
		metadataVocabularyQuery,
		['metadataVocabulary', field, libraryId],
		{ field, libraryId },
		{ enabled, staleTime: 5 * 60 * 1000 },
	)
	return { entries: data?.metadataValueCounts ?? [], isLoading }
}

/**
 * A list of values (genres or authors) as removable chips, with a picker over the values
 * the library already uses. A genre the library doesn't have yet is only added through an
 * explicit "Create" option, so a typo doesn't start a new shelf; authors can be typed freely.
 */
export default function ValueChipsField({
	field,
	libraryId,
	values,
	onChange,
	addLabel,
	disabled,
}: Props) {
	const [open, setOpen] = useState(false)
	const [search, setSearch] = useState('')
	const isGenres = field === MetadataFacetField.Genres
	const { entries, isLoading } = useVocabulary(field, libraryId, open)

	const grouped = useMemo(
		() => (isGenres ? groupVocabulary(entries, search, values) : null),
		[entries, search, values, isGenres],
	)
	const names = useMemo(() => {
		if (isGenres) return []
		const taken = new Set(values.map((v) => v.toLowerCase()))
		return [...entries]
			.filter((e) => !taken.has(e.value.toLowerCase()) && matchesSearch(e.value, search))
			.sort((a, b) => b.count - a.count || a.value.localeCompare(b.value))
			.slice(0, 40)
	}, [entries, search, values, isGenres])
	const creatable = creatableValue(search, entries, values)

	const add = (value: string) => {
		onChange([...values, value])
		setSearch('')
	}
	const remove = (value: string) => onChange(values.filter((v) => v !== value))

	const empty =
		!creatable &&
		(isGenres
			? !grouped?.genre.length && !grouped?.subgenre.length && !grouped?.facet.length
			: !names.length)

	return (
		<div className="gap-1.5 flex flex-wrap items-center">
			{values.map((value) => {
				const facet = isGenres && tagGroup(value) === 'facet'
				return (
					<span
						key={value}
						className={cn(
							'gap-1 py-0.5 pl-2.5 pr-1 text-xs min-h-7 inline-flex max-w-full items-center rounded-full border',
							facet
								? 'border-dashed border-border text-muted-foreground'
								: 'border-border bg-muted/60 text-foreground',
						)}
					>
						<span className="truncate">{value}</span>
						<button
							type="button"
							disabled={disabled}
							onClick={() => remove(value)}
							aria-label={`Remove ${value}`}
							className="size-5 pointer-coarse:size-8 inline-flex shrink-0 items-center justify-center rounded-full text-muted-foreground hover:bg-foreground/10 hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
						>
							<X className="size-3" />
						</button>
					</span>
				)
			})}

			<Popover open={open} onOpenChange={setOpen}>
				<Popover.Trigger asChild>
					<Button
						type="button"
						variant="ghost"
						size="xs"
						disabled={disabled}
						className="text-muted-foreground"
					>
						<Plus /> {addLabel}
					</Button>
				</Popover.Trigger>
				<Popover.Content
					align="start"
					className="p-0 w-[min(24rem,calc(100vw-2rem))]"
					collisionPadding={12}
				>
					<Command
						shouldFilter={false}
						onKeyDown={(event) => {
							// Escape closes the picker only, never a dialog around it
							if (event.key === 'Escape') {
								event.preventDefault()
								setOpen(false)
							}
						}}
					>
						<Command.Input
							placeholder={isGenres ? 'Search genres…' : 'Search authors…'}
							value={search}
							onValueChange={setSearch}
						/>
						<Command.List className="max-h-[min(20rem,45dvh)]">
							{grouped &&
								(['genre', 'subgenre', 'facet'] as TagGroup[]).map(
									(group) =>
										grouped[group].length > 0 && (
											<Command.Group key={group} heading={GROUP_LABEL[group]}>
												{grouped[group].map((entry) => (
													<Command.Item
														key={entry.value}
														value={entry.value}
														onSelect={() => add(entry.value)}
														className="gap-2"
													>
														<span className="flex-1 truncate">{entry.value}</span>
														<span className="text-xs text-muted-foreground tabular-nums">
															{entry.count}
														</span>
													</Command.Item>
												))}
											</Command.Group>
										),
								)}
							{!isGenres && names.length > 0 && (
								<Command.Group heading="In the library">
									{names.map((entry) => (
										<Command.Item
											key={entry.value}
											value={entry.value}
											onSelect={() => add(entry.value)}
											className="gap-2"
										>
											<span className="flex-1 truncate">{entry.value}</span>
											<span className="text-xs text-muted-foreground tabular-nums">
												{entry.count}
											</span>
										</Command.Item>
									))}
								</Command.Group>
							)}
							{creatable && (
								<Command.Group heading={isGenres ? 'New genre' : 'New author'}>
									<Command.Item
										value={`create:${creatable}`}
										onSelect={() => add(creatable)}
										className="gap-2 items-start"
									>
										{isGenres ? (
											<AlertTriangle className="mt-0.5 size-4 text-amber-500 shrink-0" />
										) : (
											<Plus className="mt-0.5 size-4 shrink-0" />
										)}
										<span className="min-w-0 flex flex-col">
											<span className="truncate">
												{isGenres ? 'Create' : 'Add'} “{creatable}”
											</span>
											{isGenres && (
												<span className="text-xs text-muted-foreground">Check the spelling</span>
											)}
										</span>
									</Command.Item>
								</Command.Group>
							)}
							{empty && (
								<div className="py-6 text-sm text-center text-muted-foreground">
									{isLoading ? 'Loading…' : 'Nothing matches'}
								</div>
							)}
						</Command.List>
					</Command>
				</Popover.Content>
			</Popover>
		</div>
	)
}
