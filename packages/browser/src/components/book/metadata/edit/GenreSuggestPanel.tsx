import { useGraphQL } from '@stump/client'
import { Button, cn } from '@stump/components'
import { MediaGenreSuggestionsQuery } from '@stump/graphql'
import { Check, Globe, Sparkles, X } from 'lucide-react'
import { useState } from 'react'

import { confidenceLevel, isVoteReason } from '@/scenes/intake/reviewState'

import { lookupLine, lookupNeedsAttention } from './onlineLookups'
import { mediaGenreSuggestionsQuery } from './queries'
import { applyPicks, defaultPicks, tagGroup } from './tags'

type Props = {
	mediaId: string
	/** The genres in the form right now */
	current: string[]
	onApply: (genres: string[]) => void
	onClose: () => void
}

type Data = MediaGenreSuggestionsQuery['mediaGenreSuggestions']
type Suggestion = Data['suggestions'][number]

const DOT = {
	high: 'bg-emerald-500 border-emerald-500',
	low: 'bg-amber-500 border-amber-500',
	medium: 'bg-sky-500 border-sky-500',
}

/** Backed by the book itself (a subject found online, its year, ...) or only by votes */
const originOf = (s: Suggestion): 'book' | 'guessed' =>
	s.reasons.some((r) => !isVoteReason(r) && r !== 'the parent of a suggested tag')
		? 'book'
		: 'guessed'

const toggle = (list: string[], value: string) =>
	list.includes(value) ? list.filter((v) => v !== value) : [...list, value]

/**
 * Genre suggestions for a book already in the library: the same scoring as "Add books"
 * (the author's and series' other books, similar descriptions, online subjects), with the
 * book's current genres judged against that evidence. Every suggestion says how sure the
 * server is and why. Nothing changes until "Apply", and nothing is saved until "Save".
 */
export default function GenreSuggestPanel({ mediaId, current, onApply, onClose }: Props) {
	const [online, setOnline] = useState(false)
	const { data, isFetching, isError } = useGraphQL(
		mediaGenreSuggestionsQuery,
		['mediaGenreSuggestions', mediaId, online],
		{ id: mediaId, online },
		{ staleTime: 60 * 1000 },
	)
	const result = data?.mediaGenreSuggestions

	// the server's default picks, set again when a new result arrives
	const [pickedFor, setPickedFor] = useState<Data>()
	const [remove, setRemove] = useState<string[]>([])
	const [add, setAdd] = useState<string[]>([])
	if (result && result !== pickedFor) {
		const picks = defaultPicks(current, result.current, result.suggestions)
		setPickedFor(result)
		setRemove(picks.remove)
		setAdd(picks.add)
	}

	const currentFits = (result?.current ?? []).filter((f) => current.includes(f.value))
	const suggestions = (result?.suggestions ?? []).filter(
		(s) => !current.some((c) => c.toLowerCase() === s.value.toLowerCase()),
	)
	const genreSuggestions = suggestions.filter((s) => s.kind !== 'FACET')
	const facetSuggestions = suggestions.filter((s) => s.kind === 'FACET')
	const changes = remove.length + add.length
	const allAdded = suggestions.length > 0 && suggestions.every((s) => add.includes(s.value))
	const unfit = currentFits.filter((f) => !f.fits && f.kind !== 'FACET')

	return (
		<section
			aria-label="Suggested genres"
			className="gap-3 p-3 flex flex-col rounded-lg border border-border bg-muted/30"
		>
			<header className="gap-2 flex items-center">
				<Sparkles className="size-4 shrink-0 text-primary" aria-hidden />
				<h3 className="text-sm font-medium flex-1">Suggested genres</h3>
				<Button
					type="button"
					variant={online ? 'secondary' : 'ghost'}
					size="xs"
					aria-pressed={online}
					title={
						online
							? 'Open Library and Google Books subjects are included'
							: 'Also look up Open Library and Google Books subjects (slower)'
					}
					onClick={() => setOnline((o) => !o)}
					disabled={isFetching}
				>
					<Globe /> {online ? 'Online on' : 'Look online'}
				</Button>
				<Button
					type="button"
					variant="ghost"
					size="icon"
					className="size-7 pointer-coarse:size-10"
					aria-label="Close the suggestions"
					onClick={onClose}
				>
					<X />
				</Button>
			</header>

			{isFetching && (
				<p className="text-xs text-muted-foreground" role="status">
					{online ? 'Looking online…' : 'Finding genres…'}
				</p>
			)}
			{isError && <p className="text-xs text-destructive">Couldn’t load suggestions. Try again.</p>}
			{result && !isFetching && result.onlineLookups.length > 0 && (
				<ul className="gap-0.5 text-xs flex flex-col" aria-label="Online lookups">
					{result.onlineLookups.map((lookup) => (
						<li
							key={lookup.source}
							className={cn(
								lookupNeedsAttention(lookup.status)
									? 'text-amber-700 dark:text-amber-400'
									: 'text-muted-foreground',
							)}
						>
							{lookupLine(lookup)}
						</li>
					))}
				</ul>
			)}

			{result && (
				<>
					{currentFits.length > 0 && (
						<div className="gap-1.5 flex flex-col">
							<p className="text-xs text-muted-foreground">Current: tap to keep or drop</p>
							<div className="gap-1.5 flex flex-wrap" role="group" aria-label="Current genres">
								{currentFits.map((fit) => {
									const dropping = remove.includes(fit.value)
									const fits = fit.fits || fit.kind === 'FACET'
									return (
										<button
											key={fit.value}
											type="button"
											aria-pressed={!dropping}
											onClick={() => setRemove((r) => toggle(r, fit.value))}
											title={
												fit.reasons.join(' · ') || (fits ? undefined : 'Nothing else points to it')
											}
											className={cn(
												'gap-1.5 px-2.5 py-1 text-xs min-h-7 pointer-coarse:min-h-9 inline-flex max-w-full items-center rounded-full border transition-colors',
												dropping
													? 'border-amber-500/60 decoration-amber-500/70 border-dashed text-muted-foreground line-through'
													: 'border-border bg-background text-foreground',
												tagGroup(fit.value) === 'facet' && !dropping && 'text-muted-foreground',
											)}
										>
											{dropping ? (
												<X className="size-3 text-amber-500 shrink-0" aria-hidden />
											) : (
												<Check className="size-3 shrink-0" aria-hidden />
											)}
											<span className="truncate">{fit.value}</span>
											{!fits && <span className="sr-only">(doesn’t fit)</span>}
										</button>
									)
								})}
							</div>
							{unfit.length > 0 && (
								<p className="text-xs text-muted-foreground">
									Struck out: nothing else points to {unfit.length > 1 ? 'them' : 'it'}.
								</p>
							)}
						</div>
					)}

					<div className="gap-1.5 flex flex-col">
						<div className="gap-2 flex items-center">
							<p className="text-xs flex-1 text-muted-foreground">Suggestions, best first</p>
							{suggestions.length > 1 && (
								<Button
									type="button"
									variant="ghost"
									size="xs"
									onClick={() => setAdd(allAdded ? [] : suggestions.map((s) => s.value))}
								>
									{allAdded ? 'Clear all' : 'Accept all'}
								</Button>
							)}
						</div>
						{suggestions.length ? (
							<ul className="gap-1 flex flex-col" aria-label="Suggested genres">
								{[...genreSuggestions, ...facetSuggestions].map((s) => (
									<SuggestionRow
										key={s.value}
										suggestion={s}
										isOn={add.includes(s.value)}
										onToggle={() => setAdd((a) => toggle(a, s.value))}
									/>
								))}
							</ul>
						) : (
							<p className="text-xs text-muted-foreground">
								Nothing to suggest.{!online && ' Try Look online.'}
							</p>
						)}
					</div>

					{result.similarBooks.length > 0 && (
						<details className="text-xs text-muted-foreground">
							<summary className="pointer-coarse:py-2 cursor-pointer select-none">
								Compared with {result.similarBooks.length} books
							</summary>
							<ul className="mt-1.5 gap-1 flex flex-col">
								{result.similarBooks.map((b) => (
									<li key={b.mediaId}>
										<span className="text-foreground">{b.title}</span> ({b.reason}):{' '}
										{b.genres.join(', ') || 'no genres'}
									</li>
								))}
							</ul>
						</details>
					)}

					<footer className="gap-2 flex items-center justify-end">
						<Button type="button" variant="ghost" size="sm" onClick={onClose}>
							Cancel
						</Button>
						<Button
							type="button"
							size="sm"
							disabled={!changes}
							onClick={() => onApply(applyPicks(current, remove, add))}
						>
							{changes ? `Apply ${changes} change${changes > 1 ? 's' : ''}` : 'No changes'}
						</Button>
					</footer>
				</>
			)}
		</section>
	)
}

/** A suggestion: a toggle with how sure the server is and why, on one or two lines */
function SuggestionRow({
	suggestion: s,
	isOn,
	onToggle,
}: {
	suggestion: Suggestion
	isOn: boolean
	onToggle: () => void
}) {
	const origin = originOf(s)
	const guessed = origin === 'guessed'
	const level = confidenceLevel(s.confidence)
	const percent = Math.round(s.confidence * 100)
	return (
		<li>
			<button
				type="button"
				aria-pressed={isOn}
				onClick={onToggle}
				className={cn(
					'gap-x-2 px-2 py-1.5 pointer-coarse:py-2 grid w-full grid-cols-[auto_minmax(0,1fr)_auto] items-center rounded-md border text-left transition-colors',
					isOn
						? guessed
							? 'border-warning/50 bg-warning/10'
							: 'border-primary/40 bg-primary/10'
						: 'border-transparent hover:bg-muted',
				)}
			>
				<span
					className={cn(
						'size-4 rounded inline-flex items-center justify-center border',
						isOn
							? 'border-primary bg-primary text-primary-foreground'
							: 'border-muted-foreground/50',
					)}
					aria-hidden
				>
					{isOn && <Check className="size-3" />}
				</span>
				<span className="min-w-0 flex flex-col">
					<span
						className={cn('text-sm line-clamp-2 break-words text-foreground', guessed && 'italic')}
					>
						{s.value}
						{tagGroup(s.value) === 'facet' && (
							<span className="text-xs ml-1.5 text-muted-foreground not-italic">facet</span>
						)}
					</span>
					<span
						className="text-xs line-clamp-2 text-muted-foreground"
						title={s.reasons.join(' · ') || undefined}
					>
						{s.reasons[0] ?? 'suggested'}
						{s.reasons.length > 1 ? ` (+${s.reasons.length - 1} more)` : ''}
					</span>
				</span>
				<span
					className="gap-1.5 text-xs inline-flex items-center text-muted-foreground tabular-nums"
					title={`${percent}% sure`}
				>
					<span
						className={cn('size-2 rounded-full border', DOT[level], guessed && 'bg-transparent')}
						aria-hidden
					/>
					{percent}%
				</span>
			</button>
		</li>
	)
}
