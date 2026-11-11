import { Badge, Button, cn, Link, NativeSelect } from '@stump/components'
import { IntakeSeriesKind, IntakeState } from '@stump/graphql'

import { usePaths } from '@/paths'

import type { IntakeItemData } from './queries'
import { parseNumber } from './reviewState'

/** Moves the item to another library: the server analyses it again for that library */
export function LibrarySwitch({
	value,
	libraries,
	onChange,
	disabled,
}: {
	value: string
	libraries: Array<{ id: string; name: string }>
	onChange: (libraryId: string) => void
	disabled?: boolean
}) {
	return (
		<label
			className="gap-1 text-xs flex flex-col items-start text-muted-foreground"
			title="Moving a book to another library analyses it again there"
		>
			Library
			<NativeSelect
				size="sm"
				className="text-base md:text-sm w-auto"
				value={value}
				disabled={disabled}
				options={libraries.map((library) => ({ label: library.name, value: library.id }))}
				onChange={(e) => {
					if (e.target.value !== value) onChange(e.target.value)
				}}
			/>
		</label>
	)
}

export function SeriesKindSwitch({
	isSeriesItem,
	value,
	onChange,
	disabled,
}: {
	isSeriesItem: boolean
	value: IntakeSeriesKind | null
	onChange: (kind: IntakeSeriesKind | null) => void
	disabled?: boolean
}) {
	const options: Array<{ label: string; value: IntakeSeriesKind | null }> = [
		...(isSeriesItem ? [] : [{ label: 'Author folder', value: null }]),
		{ label: 'Manga', value: IntakeSeriesKind.Manga },
		{ label: 'Light novel', value: IntakeSeriesKind.LightNovel },
	]
	return (
		<div
			role="radiogroup"
			aria-label="File it under"
			className="p-0.5 inline-flex max-w-full overflow-x-auto rounded-lg border border-border bg-background"
		>
			{options.map((option) => {
				const active = option.value === value
				return (
					<button
						key={option.label}
						type="button"
						role="radio"
						aria-checked={active}
						disabled={disabled}
						onClick={() => !active && onChange(option.value)}
						className={cn(
							'px-2.5 h-7 text-xs rounded-md whitespace-nowrap transition-colors disabled:opacity-50',
							active
								? 'font-medium bg-muted text-foreground'
								: 'text-muted-foreground hover:text-foreground',
						)}
					>
						{option.label}
					</button>
				)
			})}
		</div>
	)
}

export function VolumeList({
	item,
	onChange,
	disabled,
}: {
	item: IntakeItemData
	onChange: (volumes: Array<number | null>) => void
	disabled?: boolean
}) {
	const volumes = item.decision?.volumes ?? []
	const files = item.destination?.files ?? []
	return (
		<section className="gap-1.5 flex flex-col" aria-label="Volumes">
			<span className="text-xs text-muted-foreground">Volumes</span>
			<ul className="divide-y divide-border rounded-lg border border-border bg-card">
				{item.files.map((file, index) => {
					const target = files[index]
					return (
						<li key={file.originalName} className="gap-3 px-3 py-2 flex items-center">
							<label className="gap-1 flex items-center">
								<span className="text-xs text-muted-foreground">#</span>
								<VolumeInput
									value={volumes[index]}
									disabled={disabled}
									onCommit={(volume) => {
										const next = item.files.map((_, i) => volumes[i] ?? null)
										next[index] = volume
										onChange(next)
									}}
								/>
							</label>
							<div className="min-w-0 flex flex-1 flex-col">
								<span className="text-sm truncate" title={target?.relativePath}>
									{target?.relativePath.split('/').pop() ?? file.originalName}
								</span>
								<span className="text-xs truncate text-muted-foreground" title={file.originalName}>
									{file.originalName}
								</span>
							</div>
							{target?.exists && (
								<Badge variant="error" size="xs">
									exists
								</Badge>
							)}
						</li>
					)
				})}
			</ul>
		</section>
	)
}

function VolumeInput({
	value,
	onCommit,
	disabled,
}: {
	value?: number | null
	onCommit: (value: number | null) => void
	disabled?: boolean
}) {
	return (
		<input
			key={value ?? 'none'}
			defaultValue={value ?? ''}
			inputMode="decimal"
			disabled={disabled}
			aria-label="Volume number"
			className="h-8 w-14 px-2 text-base md:text-sm rounded-md border border-border bg-input/30"
			onBlur={(e) => {
				const next = parseNumber(e.target.value) ?? null
				if (next !== (value ?? null)) onCommit(next)
			}}
			onKeyDown={(e) => {
				if (e.key === 'Enter') (e.target as HTMLInputElement).blur()
			}}
		/>
	)
}

/** Books of the library like this one, with why: a hint for the tags, and a check for a duplicate */
export function SimilarBooks({
	item,
	onLooksRight,
	disabled,
}: {
	item: IntakeItemData
	/** Confirms the tags as they are, for a book the server wasn't sure about */
	onLooksRight: () => void
	disabled?: boolean
}) {
	const paths = usePaths()
	const similar = item.proposal?.similarBooks ?? []
	if (!similar.length) return null
	return (
		<section className="gap-1.5 flex flex-col" aria-label="Similar books">
			<div className="gap-2 flex flex-wrap items-center justify-between">
				<span className="text-xs text-muted-foreground">Similar books</span>
				{item.state === IntakeState.NeedsReview && item.problems.length === 0 && (
					<Button size="xs" variant="secondary" disabled={disabled} onClick={onLooksRight}>
						Looks right
					</Button>
				)}
			</div>
			<ul className="gap-1 flex flex-col">
				{similar.slice(0, 3).map((book) => (
					<li key={book.mediaId} className="text-sm min-w-0">
						<Link to={paths.bookOverview(book.mediaId)} className="font-medium">
							{book.title}
						</Link>
						{book.authors.length > 0 && (
							<span className="text-muted-foreground"> · {book.authors.join(', ')}</span>
						)}
						<span className="text-xs block truncate text-muted-foreground">
							{book.reason}
							{book.genres.length > 0 && ` — ${book.genres.slice(0, 4).join(', ')}`}
						</span>
					</li>
				))}
			</ul>
		</section>
	)
}
