import {
	Alert,
	AlertDescription,
	AlertTitle,
	Badge,
	Button,
	cn,
	DropdownMenu,
	Link,
} from '@stump/components'
import { IntakeKind, IntakeReviewInput, IntakeSeriesKind, IntakeState } from '@stump/graphql'
import {
	AlertTriangle,
	BookOpen,
	CircleAlert,
	Copy,
	EllipsisVertical,
	FileText,
	FolderOpen,
	Globe,
	Images,
	Layers,
	RotateCw,
	Trash2,
} from 'lucide-react'
import { useMemo } from 'react'

import { usePaths } from '@/paths'

import FolderPicker from './FolderPicker'
import type { IntakeItemData } from './queries'
import { ReviewField } from './ReviewField'
import {
	fieldSource,
	formatAuthors,
	genreChips,
	isReady,
	needsAttention,
	parseAuthors,
	parseNumber,
	SERIES_KIND_LABELS,
} from './reviewState'
import TagPicker from './TagPicker'

type Props = {
	item: IntakeItemData
	onReview: (id: string, input: IntakeReviewInput) => void
	onAccept: (item: IntakeItemData) => void
	onDiscard: (ids: string[]) => void
	onReanalyse: (id: string, online?: boolean) => void
	isBusy?: boolean
}

export function FormatIcon({
	format,
	series,
	className,
}: {
	format?: string
	series?: boolean
	className?: string
}) {
	const Icon = series
		? Layers
		: format === 'pdf'
			? FileText
			: format === 'cbz' || format === 'cbr'
				? Images
				: BookOpen
	return <Icon className={className} aria-hidden />
}

const STATE_BADGE: Partial<
	Record<IntakeState, { label: string; variant: 'success' | 'warning' | 'error' }>
> = {
	[IntakeState.Ready]: { label: 'Ready', variant: 'success' },
	[IntakeState.NeedsReview]: { label: 'Check this', variant: 'warning' },
	[IntakeState.Failed]: { label: 'Failed', variant: 'error' },
}

/** One inbox item to review: what it is, where it goes, its tags, and what blocks it */
export default function IntakeItemCard({
	item,
	onReview,
	onAccept,
	onDiscard,
	onReanalyse,
	isBusy,
}: Props) {
	const paths = usePaths()
	const { proposal, decision, destination } = item
	const meta = decision?.metadata
	const isSeriesItem = item.kind === IntakeKind.Series
	const seriesKind = decision?.seriesKind ?? null
	const filedAsSeries = isSeriesItem || !!seriesKind
	const attention = needsAttention(item)
	const ready = isReady(item)
	const review = (input: IntakeReviewInput) => onReview(item.id, input)

	const chips = useMemo(() => genreChips(item), [item])
	const chosenGenres = meta?.genres ?? []
	const firstFormat = item.files[0]?.format
	const badge = STATE_BADGE[item.state]
	const title = (filedAsSeries ? meta?.series : meta?.title) || item.name
	const singleFile = destination?.files.length === 1 ? destination.files[0] : undefined

	const folderCandidates = (filedAsSeries ? proposal?.seriesFolders : proposal?.authorFolders) ?? []

	return (
		<article
			className={cn(
				'gap-4 p-4 flex flex-col rounded-xl border bg-card',
				attention ? 'border-warning/50 ring-1 ring-warning/20' : 'border-border',
			)}
			aria-label={title}
		>
			{/* header */}
			<div className="gap-3 flex items-start">
				<div className="h-16 w-12 flex shrink-0 items-center justify-center rounded-md bg-muted text-muted-foreground">
					<FormatIcon format={firstFormat} series={isSeriesItem} className="h-6 w-6" />
				</div>
				<div className="min-w-0 flex flex-1 flex-col">
					<div className="gap-2 flex flex-wrap items-center">
						<h3 className="font-semibold leading-tight line-clamp-2 break-words">{title}</h3>
						{badge && (
							<Badge variant={badge.variant} size="xs">
								{badge.label}
							</Badge>
						)}
						{isSeriesItem && (
							<Badge variant="secondary" size="xs">
								{item.files.length} volumes
							</Badge>
						)}
					</div>
					<span className="text-xs truncate text-muted-foreground" title={item.name}>
						{item.name} · {item.libraryName}
						{proposal && !proposal.lookedUpOnline && ' · not looked up online'}
					</span>
				</div>
				<DropdownMenu
					align="end"
					trigger={
						<Button variant="ghost" size="icon" className="shrink-0" aria-label="More actions">
							<EllipsisVertical className="h-4 w-4" />
						</Button>
					}
					groups={[
						{
							items: [
								{
									label: 'Analyse again',
									leftIcon: <RotateCw className="mr-2 h-4 w-4" />,
									onClick: () => onReanalyse(item.id),
								},
								{
									label: 'Look it up online again',
									leftIcon: <Globe className="mr-2 h-4 w-4" />,
									onClick: () => onReanalyse(item.id, true),
								},
								{
									label: 'Discard',
									leftIcon: <Trash2 className="mr-2 h-4 w-4" />,
									onClick: () => onDiscard([item.id]),
								},
							],
						},
					]}
				/>
			</div>

			{item.state === IntakeState.Failed && (
				<Alert variant="destructive">
					<CircleAlert />
					<AlertTitle>It could not be analysed</AlertTitle>
					<AlertDescription>{item.error || 'Unknown error'}</AlertDescription>
				</Alert>
			)}

			{decision && (
				<>
					{/* where it goes */}
					<div className="gap-2 p-3 flex flex-col rounded-lg bg-muted/50">
						<div className="gap-2 flex items-start">
							<FolderOpen className="mt-0.5 h-4 w-4 shrink-0 text-muted-foreground" aria-hidden />
							<div className="min-w-0 flex-1">
								<p className="font-mono text-sm break-all">
									<span className="text-muted-foreground">{destination?.relativeDir || '…'}/</span>
									{singleFile && (
										<span className="font-medium">{singleFile.relativePath.split('/').pop()}</span>
									)}
								</p>
								<div className="mt-1 gap-2 flex flex-wrap items-center">
									{destination && !destination.dirExists && (
										<Badge variant="warning" size="xs">
											new folder
										</Badge>
									)}
									{destination?.dirExists && (
										<Badge variant="success" size="xs">
											existing folder
										</Badge>
									)}
									{filedAsSeries && seriesKind && (
										<span className="text-xs text-muted-foreground">
											{SERIES_KIND_LABELS[seriesKind]} series
										</span>
									)}
								</div>
							</div>
						</div>

						<div className="gap-2 flex flex-wrap items-center">
							<FolderPicker
								libraryId={item.libraryId}
								seriesKind={seriesKind}
								candidates={folderCandidates}
								value={decision.folder}
								onSelect={(folder) => review({ folder })}
								disabled={isBusy}
							/>
							<SeriesKindSwitch
								isSeriesItem={isSeriesItem}
								value={seriesKind}
								disabled={isBusy}
								onChange={(kind) => review(kind ? { seriesKind: kind } : { clearSeriesKind: true })}
							/>
						</div>
					</div>

					{/* what it is */}
					<div className="gap-3 sm:grid-cols-6 grid grid-cols-2">
						{filedAsSeries ? (
							<>
								<ReviewField
									className="sm:col-span-4 col-span-2"
									label="Series"
									value={meta?.series ?? ''}
									source={fieldSource(meta, 'series')}
									placeholder="Series name"
									invalid={!meta?.series}
									onCommit={(series) => review({ series })}
									disabled={isBusy}
								/>
								{!isSeriesItem && (
									<ReviewField
										label="Volume"
										value={meta?.seriesIndex?.toString() ?? ''}
										source={fieldSource(meta, 'seriesIndex')}
										inputMode="decimal"
										onCommit={(value) => {
											const seriesIndex = parseNumber(value)
											if (seriesIndex !== undefined) review({ seriesIndex })
										}}
										disabled={isBusy}
									/>
								)}
								<ReviewField
									label="Language"
									value={meta?.language ?? ''}
									source={fieldSource(meta, 'language')}
									placeholder="en"
									onCommit={(language) => review({ language })}
									disabled={isBusy}
								/>
							</>
						) : (
							<>
								<ReviewField
									className="sm:col-span-6 col-span-2"
									label="Title"
									value={meta?.title ?? ''}
									source={fieldSource(meta, 'title')}
									invalid={!meta?.title}
									onCommit={(title) => review({ title })}
									disabled={isBusy}
								/>
								<ReviewField
									className="sm:col-span-6 col-span-2"
									label="Authors"
									hint="Separate several authors with ;"
									value={formatAuthors(meta?.authors ?? [])}
									source={fieldSource(meta, 'authors')}
									invalid={!meta?.authors.length}
									placeholder="e.g. Umberto Eco"
									onCommit={(value) => review({ authors: parseAuthors(value) })}
									disabled={isBusy}
								/>
								<ReviewField
									className="sm:col-span-3 col-span-2"
									label="Series"
									value={meta?.series ?? ''}
									source={fieldSource(meta, 'series')}
									placeholder="None"
									onCommit={(series) => review({ series })}
									disabled={isBusy}
								/>
								<ReviewField
									label="#"
									value={meta?.seriesIndex?.toString() ?? ''}
									source={fieldSource(meta, 'seriesIndex')}
									inputMode="decimal"
									onCommit={(value) => {
										const seriesIndex = parseNumber(value)
										if (seriesIndex !== undefined) review({ seriesIndex })
									}}
									disabled={isBusy}
								/>
								<ReviewField
									label="Language"
									value={meta?.language ?? ''}
									source={fieldSource(meta, 'language')}
									placeholder="en"
									onCommit={(language) => review({ language })}
									disabled={isBusy}
								/>
								<ReviewField
									label="Year"
									value={meta?.year?.toString() ?? ''}
									source={fieldSource(meta, 'year')}
									inputMode="numeric"
									onCommit={(value) => {
										const year = parseNumber(value)
										if (year !== undefined) review({ year: Math.round(year) })
									}}
									disabled={isBusy}
								/>
							</>
						)}
					</div>

					{/* volumes of a series */}
					{(isSeriesItem || item.files.length > 1) && destination && (
						<VolumeList item={item} disabled={isBusy} onChange={(volumes) => review({ volumes })} />
					)}

					{/* tags */}
					{!filedAsSeries || chips.length > 0 ? (
						<section className="gap-2 flex flex-col" aria-label="Tags">
							<div className="gap-2 flex items-center">
								<span className="text-xs text-muted-foreground">Genres and tags</span>
								<span className="text-xs text-muted-foreground/70">
									{chosenGenres.length ? `${chosenGenres.length} chosen` : 'none chosen'}
								</span>
							</div>
							<TagPicker
								libraryId={item.libraryId}
								chips={chips}
								chosen={chosenGenres}
								onChange={(genres) => review({ genres })}
								disabled={isBusy}
							/>
						</section>
					) : null}

					{/* preview */}
					{!!proposal?.similarBooks.length && (
						<section className="gap-1.5 p-3 flex flex-col rounded-lg border border-dashed border-border">
							<div className="gap-2 flex flex-wrap items-center justify-between">
								<span className="text-sm font-medium">Does this look right?</span>
								{item.state === IntakeState.NeedsReview && item.problems.length === 0 && (
									<Button
										size="xs"
										variant="secondary"
										disabled={isBusy}
										onClick={() => review({ genres: chosenGenres })}
									>
										Looks right
									</Button>
								)}
							</div>
							<span className="text-xs text-muted-foreground">
								Similar books already in the library:
							</span>
							<ul className="gap-1 flex flex-col">
								{proposal.similarBooks.slice(0, 3).map((book) => (
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
					)}

					{/* duplicates, warnings, problems */}
					{!!proposal?.duplicates.length && (
						<Alert variant="warning">
							<Copy />
							<AlertTitle>Already in the library?</AlertTitle>
							<AlertDescription>
								<ul className="gap-1 flex flex-col">
									{proposal.duplicates.map((dup) => (
										<li key={`${dup.mediaId}:${dup.fileName}`}>
											<Link to={paths.bookOverview(dup.mediaId)}>{dup.title}</Link>{' '}
											<span className="text-muted-foreground">
												(
												{dup.reason === 'SAME_FILE'
													? 'the very same file'
													: 'same author and title'}
												{' — '}
												<span className="break-all">{dup.path}</span>)
											</span>
										</li>
									))}
								</ul>
							</AlertDescription>
						</Alert>
					)}

					{!!proposal?.warnings.length && (
						<ul className="gap-1 text-xs flex flex-col text-muted-foreground">
							{proposal.warnings.map((warning) => (
								<li key={warning} className="gap-1.5 flex items-start">
									<AlertTriangle className="h-3.5 w-3.5 mt-px shrink-0 text-warning" aria-hidden />
									{warning}
								</li>
							))}
						</ul>
					)}

					{item.problems.length > 0 && (
						<Alert variant="destructive">
							<CircleAlert />
							<AlertTitle>Before it can be added</AlertTitle>
							<AlertDescription>
								<ul className="pl-4 list-disc">
									{item.problems.map((problem) => (
										<li key={problem}>{problem}</li>
									))}
								</ul>
							</AlertDescription>
						</Alert>
					)}

					{item.error && item.state !== IntakeState.Failed && (
						<Alert variant="destructive">
							<CircleAlert />
							<AlertTitle>Adding it failed</AlertTitle>
							<AlertDescription>{item.error}</AlertDescription>
						</Alert>
					)}
				</>
			)}

			{/* actions */}
			<div className="gap-2 flex flex-wrap items-center justify-end">
				<Button variant="ghost" size="sm" onClick={() => onDiscard([item.id])} disabled={isBusy}>
					Discard
				</Button>
				<Button
					size="sm"
					onClick={() => onAccept(item)}
					disabled={
						isBusy || !decision || item.problems.length > 0 || item.state === IntakeState.Failed
					}
					title={item.problems[0]}
				>
					{item.state === IntakeState.NeedsReview
						? 'Looks right, add it'
						: ready && !attention
							? 'Add to library'
							: 'Add anyway'}
				</Button>
			</div>
		</article>
	)
}

function SeriesKindSwitch({
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

function VolumeList({
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
			<ul className="divide-y divide-border rounded-lg border border-border">
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
