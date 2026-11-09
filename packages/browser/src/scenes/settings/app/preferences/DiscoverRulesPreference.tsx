import { useGraphQL, useGraphQLMutation, useSDK, useSuspenseGraphQL } from '@stump/client'
import { Button, cn, Command, NativeSelect, NewCard, Popover, RawSwitch } from '@stump/components'
import { DiscoverRulesInput, graphql, MetadataFacetField } from '@stump/graphql'
import { useQueryClient } from '@tanstack/react-query'
import { Plus, X } from 'lucide-react'
import { useMemo, useState } from 'react'
import { useDebouncedValue } from 'rooks'
import { toast } from 'sonner'

const rulesQuery = graphql(`
	query DiscoverRulesPreference {
		myDiscoverRules {
			unreadOnly
			excludedTags
			excludedGenres
			excludedLibraryIds
			minRating
		}
	}
`)

const optionsQuery = graphql(`
	query DiscoverRulesOptions($field: MetadataFacetField!) {
		tags {
			id
			name
		}
		libraries(pagination: { none: { unpaginated: true } }) {
			nodes {
				id
				name
				emoji
			}
		}
		metadataValueCounts(field: $field) {
			value
			count
		}
	}
`)

const poolQuery = graphql(`
	query DiscoverPoolSize($rules: DiscoverRulesInput) {
		discoverPoolSize(rules: $rules)
	}
`)

const updateMutation = graphql(`
	mutation UpdateDiscoverRules($input: DiscoverRulesInput!) {
		updateMyDiscoverRules(input: $input) {
			unreadOnly
			excludedTags
			excludedGenres
			excludedLibraryIds
			minRating
		}
	}
`)

export const DEFAULT_DISCOVER_RULES: DiscoverRulesInput = {
	unreadOnly: true,
	excludedTags: ['No picks'],
	excludedGenres: [],
	excludedLibraryIds: [],
	minRating: null,
}

/** One-click genre rules, offered when the libraries have them */
const SUGGESTED_GENRES = [
	'Level: Middle School',
	'Level: High School',
	'Form: Textbook',
	'Form: Workbook & Solutions',
	'Form: Reference Work',
	'Reference: *',
	'Language & Writing: Dictionaries',
	'Language & Writing: Graded Readers',
	'Manga & Comics',
	'Light Novels',
]

const RATINGS = [
	{ label: 'Any', value: '' },
	{ label: '2+', value: '2' },
	{ label: '3+', value: '3' },
	{ label: '3.5+', value: '3.5' },
	{ label: '4+', value: '4' },
]

/** A row whose control (chips) sits under its label, across the card */
const STACKED = 'lg:flex-col lg:items-stretch lg:gap-2.5'

type Option = { value: string; label: string; count?: number; hint?: string }

const sameRules = (a: DiscoverRulesInput, b: DiscoverRulesInput) =>
	JSON.stringify(a) === JSON.stringify(b)

const toInput = (rules: DiscoverRulesInput): DiscoverRulesInput => ({
	unreadOnly: rules.unreadOnly,
	excludedTags: rules.excludedTags,
	excludedGenres: rules.excludedGenres,
	excludedLibraryIds: rules.excludedLibraryIds,
	minRating: rules.minRating ?? null,
})

/**
 * Settings → Preferences → Picks of the week: what the home's picks and Surprise me can
 * choose from. Changes are previewed as the number of books left, and saved together.
 */
export default function DiscoverRulesPreference() {
	const { sdk } = useSDK()
	const client = useQueryClient()
	const { data } = useSuspenseGraphQL(rulesQuery, sdk.cacheKey('discoverRules'))
	const saved = useMemo(() => toInput(data.myDiscoverRules), [data])
	const [draft, setDraft] = useState<DiscoverRulesInput>(saved)
	const isDirty = !sameRules(draft, saved)

	const { data: options } = useGraphQL(
		optionsQuery,
		['discoverRulesOptions'],
		{ field: MetadataFacetField.Genres },
		{ staleTime: 5 * 60 * 1000 },
	)

	const [debouncedDraft] = useDebouncedValue(draft, 300)
	const previewRules = debouncedDraft ?? draft
	const { data: pool, isFetching: isCounting } = useGraphQL(
		poolQuery,
		sdk.cacheKey('discoverPoolSize', [previewRules]),
		{ rules: previewRules },
		{ placeholderData: (previous) => previous },
	)

	const { mutateAsync, isPending } = useGraphQLMutation(updateMutation)
	const save = async () => {
		try {
			const result = await mutateAsync({ input: draft })
			const rules = toInput(result.updateMyDiscoverRules)
			client.setQueryData(sdk.cacheKey('discoverRules'), { myDiscoverRules: rules })
			setDraft(rules)
			// the picks follow the new rules right away
			await client.invalidateQueries({ queryKey: sdk.cacheKey('randomMedia') })
			toast.success('Saved')
		} catch (error) {
			toast.error("Couldn't save", {
				description: error instanceof Error ? error.message : undefined,
			})
		}
	}

	const set = (patch: Partial<DiscoverRulesInput>) =>
		setDraft((current) => ({ ...current, ...patch }))

	const tagOptions = useMemo<Option[]>(
		() =>
			(options?.tags ?? [])
				.map((tag) => ({ value: tag.name, label: tag.name }))
				.sort((a, b) => a.label.localeCompare(b.label)),
		[options?.tags],
	)
	const libraryOptions = useMemo<Option[]>(
		() =>
			(options?.libraries.nodes ?? []).map((library) => ({
				value: library.id,
				label: `${library.emoji ? `${library.emoji} ` : ''}${library.name}`,
			})),
		[options?.libraries.nodes],
	)
	const genreOptions = useMemo<Option[]>(
		() => genreChoices(options?.metadataValueCounts ?? []),
		[options?.metadataValueCounts],
	)
	const suggested = useMemo(() => {
		const known = new Map(genreOptions.map((option) => [option.value, option]))
		return SUGGESTED_GENRES.map((value) => known.get(value)).filter(
			(option): option is Option => !!option && !draft.excludedGenres?.includes(option.value),
		)
	}, [genreOptions, draft.excludedGenres])

	const tags = draft.excludedTags ?? []
	const genres = draft.excludedGenres ?? []
	const libraries = draft.excludedLibraryIds ?? []

	return (
		<NewCard label="Picks of the week" description="What Picks and Surprise me choose from">
			<NewCard.Row
				label="Unread only"
				onClick={() => set({ unreadOnly: !draft.unreadOnly })}
				className="flex-row items-center justify-between"
			>
				<RawSwitch
					aria-label="Unread only"
					checked={!!draft.unreadOnly}
					onCheckedChange={(checked) => set({ unreadOnly: checked })}
				/>
			</NewCard.Row>

			<NewCard.Row label="Skip tags" className={STACKED}>
				<RuleChips
					label="tag"
					values={tags}
					options={tagOptions}
					onChange={(excludedTags) => set({ excludedTags })}
					typedHint="Add"
					emptyText="No other tags. Manga and the like are genres."
					elsewhere={{
						heading: 'Genres',
						options: genreOptions,
						taken: genres,
						onPick: (value) => set({ excludedGenres: [...genres, value] }),
					}}
				/>
			</NewCard.Row>

			<NewCard.Row
				label="Skip genres"
				className={STACKED}
				description="End with * for a whole family"
			>
				<div className="gap-2 flex flex-col">
					<RuleChips
						label="genre"
						values={genres}
						options={genreOptions}
						onChange={(excludedGenres) => set({ excludedGenres })}
						typedHint="Add"
						elsewhere={{
							heading: 'Tags',
							options: tagOptions,
							taken: tags,
							onPick: (value) => set({ excludedTags: [...tags, value] }),
						}}
					/>
					{suggested.length > 0 && (
						<div className="gap-1.5 flex flex-wrap items-center">
							{suggested.map((option) => (
								<button
									key={option.value}
									type="button"
									onClick={() => set({ excludedGenres: [...genres, option.value] })}
									className="gap-1 px-2 py-0.5 text-xs inline-flex items-center rounded-full border border-dashed border-border text-muted-foreground hover:border-ring hover:text-foreground"
								>
									<Plus className="size-3" /> {option.value}
									<span className="tabular-nums opacity-70">{option.count}</span>
								</button>
							))}
						</div>
					)}
				</div>
			</NewCard.Row>

			<NewCard.Row label="Skip libraries" className={STACKED}>
				<RuleChips
					label="library"
					values={libraries}
					options={libraryOptions}
					onChange={(excludedLibraryIds) => set({ excludedLibraryIds })}
				/>
			</NewCard.Row>

			<NewCard.Row
				label="Minimum rating"
				className="sm:flex-row sm:items-center sm:justify-between"
			>
				<NativeSelect
					aria-label="Minimum rating"
					size="sm"
					className="sm:w-40"
					options={RATINGS}
					value={draft.minRating != null ? String(draft.minRating) : ''}
					onChange={(event) =>
						set({ minRating: event.target.value ? Number(event.target.value) : null })
					}
				/>
			</NewCard.Row>

			<div className="gap-3 px-4 py-3 flex flex-wrap items-center justify-between border-t border-border">
				<span
					className={cn('text-sm text-muted-foreground', isCounting && 'opacity-60')}
					aria-live="polite"
				>
					{pool ? `${pool.discoverPoolSize.toLocaleString()} books` : ' '}
				</span>
				<div className="gap-2 flex">
					<Button
						type="button"
						size="sm"
						variant="ghost"
						disabled={isPending || sameRules(draft, DEFAULT_DISCOVER_RULES)}
						onClick={() => setDraft(DEFAULT_DISCOVER_RULES)}
					>
						Defaults
					</Button>
					<Button
						type="button"
						size="sm"
						variant="ghost"
						disabled={isPending || !isDirty}
						onClick={() => setDraft(saved)}
					>
						Cancel
					</Button>
					<Button type="button" size="sm" disabled={isPending || !isDirty} onClick={save}>
						Save
					</Button>
				</div>
			</div>
		</NewCard>
	)
}

/** The libraries' genres with counts, then each `Family: *` with its total */
function genreChoices(entries: { value: string; count: number }[]): Option[] {
	const families = new Map<string, number>()
	for (const { value, count } of entries) {
		const index = value.indexOf(': ')
		if (index > 0) {
			const family = `${value.slice(0, index)}: *`
			families.set(family, (families.get(family) ?? 0) + count)
		}
	}
	return [
		...[...families].map(([value, count]) => ({
			value,
			label: value,
			count,
			hint: 'family',
		})),
		...entries.map(({ value, count }) => ({ value, label: value, count })),
	]
}

type ChipsProps = {
	/** What a value is, e.g. "genre" (for the button and labels) */
	label: string
	values: string[]
	options: Option[]
	onChange: (values: string[]) => void
	/** Offer what was typed when no option has it, with this text before it */
	typedHint?: string
	/** What to say when there is nothing left to add and nothing typed */
	emptyText?: string
	/**
	 * Matches from the other list (e.g. genres while searching tags), so a value looked for in
	 * the wrong place is still found; picking one adds it there
	 */
	elsewhere?: {
		heading: string
		options: Option[]
		taken: string[]
		onPick: (value: string) => void
	}
}

function RuleChips({
	label,
	values,
	options,
	onChange,
	typedHint,
	emptyText,
	elsewhere,
}: ChipsProps) {
	const [open, setOpen] = useState(false)
	const [search, setSearch] = useState('')
	const labelOf = (value: string) => options.find((o) => o.value === value)?.label ?? value

	const term = search.trim().toLowerCase()
	const chosen = new Set(values.map((value) => value.toLowerCase()))
	const shown = options
		.filter((o) => !chosen.has(o.value.toLowerCase()) && o.label.toLowerCase().includes(term))
		.sort((a, b) => (b.count ?? 0) - (a.count ?? 0))
		.slice(0, 60)
	const takenElsewhere = new Set(elsewhere?.taken.map((value) => value.toLowerCase()))
	const shownElsewhere = term
		? (elsewhere?.options ?? [])
				.filter(
					(o) => !takenElsewhere.has(o.value.toLowerCase()) && o.label.toLowerCase().includes(term),
				)
				.sort((a, b) => (b.count ?? 0) - (a.count ?? 0))
				.slice(0, 8)
		: []
	const typed = search.trim()
	const canUseTyped =
		!!typedHint &&
		!!typed &&
		!chosen.has(typed.toLowerCase()) &&
		!options.some((o) => o.value.toLowerCase() === typed.toLowerCase())

	const add = (value: string) => {
		onChange([...values, value])
		setSearch('')
	}

	return (
		<div className="gap-1.5 flex flex-wrap items-center">
			{values.map((value) => (
				<span
					key={value}
					className="gap-1 py-0.5 pr-1 pl-2.5 text-xs min-h-7 inline-flex max-w-full items-center rounded-full border border-border bg-muted/60 text-foreground"
				>
					<span className="truncate">{labelOf(value)}</span>
					<button
						type="button"
						onClick={() => onChange(values.filter((v) => v !== value))}
						aria-label={`Remove ${labelOf(value)}`}
						className="size-5 pointer-coarse:size-8 inline-flex shrink-0 items-center justify-center rounded-full text-muted-foreground hover:bg-foreground/10 hover:text-foreground"
					>
						<X className="size-3" />
					</button>
				</span>
			))}
			<Popover open={open} onOpenChange={setOpen}>
				<Popover.Trigger asChild>
					<Button
						type="button"
						variant="ghost"
						size="xs"
						className="text-muted-foreground"
						aria-label={`Add a ${label}`}
					>
						<Plus /> Add
					</Button>
				</Popover.Trigger>
				<Popover.Content
					align="start"
					className="p-0 w-[min(24rem,calc(100vw-2rem))]"
					collisionPadding={12}
				>
					<Command shouldFilter={false}>
						<Command.Input
							placeholder={`Search ${label}s…`}
							value={search}
							onValueChange={setSearch}
						/>
						<Command.List className="max-h-[min(20rem,45dvh)]">
							{shown.map((option) => (
								<Command.Item
									key={option.value}
									value={option.value}
									onSelect={() => add(option.value)}
									className="gap-2"
								>
									<span className="flex-1 truncate">{option.label}</span>
									{option.hint && (
										<span className="text-[10px] text-muted-foreground uppercase">
											{option.hint}
										</span>
									)}
									{option.count != null && (
										<span className="text-xs text-muted-foreground tabular-nums">
											{option.count}
										</span>
									)}
								</Command.Item>
							))}
							{canUseTyped && (
								<Command.Item value={`typed:${typed}`} onSelect={() => add(typed)}>
									{typedHint} “{typed}”
								</Command.Item>
							)}
							{elsewhere && shownElsewhere.length > 0 && (
								<Command.Group heading={elsewhere.heading}>
									{shownElsewhere.map((option) => (
										<Command.Item
											key={`elsewhere:${option.value}`}
											value={`elsewhere:${option.value}`}
											onSelect={() => {
												elsewhere.onPick(option.value)
												setSearch('')
												setOpen(false)
											}}
											className="gap-2"
										>
											<span className="flex-1 truncate">{option.label}</span>
											{option.count != null && (
												<span className="text-xs text-muted-foreground tabular-nums">
													{option.count}
												</span>
											)}
										</Command.Item>
									))}
								</Command.Group>
							)}
							{!shown.length && !canUseTyped && !shownElsewhere.length && (
								<p className="px-4 py-6 text-sm text-center text-muted-foreground">
									{(!term && emptyText) || 'Nothing to add'}
								</p>
							)}
						</Command.List>
					</Command>
				</Popover.Content>
			</Popover>
		</div>
	)
}
