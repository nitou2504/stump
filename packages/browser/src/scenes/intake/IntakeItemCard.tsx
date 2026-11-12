import { Badge, Button, cn, DropdownMenu } from '@stump/components'
import { IntakeKind, IntakeReviewInput, IntakeState } from '@stump/graphql'
import { EllipsisVertical, FolderOpen, Globe, RotateCw, Trash2 } from 'lucide-react'
import { useMemo, useState } from 'react'

import { useTranslate } from '@/hooks/useTranslate'

import AgeRatingField from './AgeRatingField'
import CoverTile from './CoverTile'
import FolderPicker from './FolderPicker'
import { formatLabel } from './FormatIcon'
import { BlockingNotices, CardNotices } from './IntakeNotices'
import IntakeStatusChip from './IntakeStatus'
import { LibrarySwitch, SeriesKindSwitch, SimilarBooks, VolumeList } from './ItemDetails'
import MoreDetails from './MoreDetails'
import type { IntakeItemData } from './queries'
import { ReviewField, ReviewTextArea } from './ReviewField'
import {
	type AgeLimit,
	effectiveGenres,
	fieldSource,
	formatAuthors,
	genreChips,
	isReady,
	isSentBack,
	needsAttention,
	parseAuthors,
	parseNumber,
	SERIES_KIND_LABELS,
	typedSeriesKind,
} from './reviewState'
import SendBackDialog from './SendBackDialog'
import TagPicker from './TagPicker'
import { useTagReviewStore } from './tagReviewStore'
import UploaderRow from './UploaderRow'

/** The longest description the server takes from somebody who needs approval */
const DESCRIPTION_MAX = 20_000

/**
 * Who is looking, which decides what the card shows and what its buttons do:
 * - `file`: somebody who files books themselves, with their own book (add it to the library)
 * - `submit`: somebody who may not: no folders, kinds or destinations (they are the approver's),
 *   and the book is sent for approval
 * - `review`: an approver with a book somebody sent: the whole card, to approve or send back
 */
export type IntakeCardMode = 'file' | 'submit' | 'review'

type Props = {
	item: IntakeItemData
	/** The libraries the user may add books to (an item can be moved to any of them) */
	libraries: Array<{ id: string; name: string }>
	onChangeLibrary: (id: string, libraryId: string) => void
	onReview: (id: string, input: IntakeReviewInput) => void
	/** `file`: files the item with these tags (sent first when they differ from the server's) */
	onAccept?: (item: IntakeItemData, genres: string[]) => void
	/** `submit`: sends the item for approval with these tags (sent first when they differ) */
	onSubmit?: (item: IntakeItemData, genres: string[]) => void
	/** `review`: approves the item with these tags (sent first when they differ from the server's) */
	onApprove?: (item: IntakeItemData, genres: string[]) => void
	/** `review`: sends the item back to its uploader with a note; `done` closes the note's dialog */
	onSendBack?: (id: string, note: string, done: () => void) => void
	/** `review`: the age limit of the uploader, for the warning that a rating hides the book */
	uploaderAge?: AgeLimit | null
	/** Not for `review`: an approver can't throw away what somebody else sent */
	onDiscard?: (ids: string[]) => void
	onReanalyse: (id: string, online?: boolean) => void
	isBusy?: boolean
	/** Who is looking (see {@link IntakeCardMode}) */
	mode?: IntakeCardMode
	/** `review`: a send-back is being saved */
	isSendingBack?: boolean
}

/**
 * One book of the inbox as a card: its cover, title, author and where it stands up top; what a
 * person is asked to check (title, author, series, tags, description) in the middle; everything
 * else (folder, library, language, year, publisher, age rating, volumes) behind *More details*;
 * and one button for the next step at the bottom.
 */
export default function IntakeItemCard({
	item,
	libraries,
	onChangeLibrary,
	onReview,
	onAccept,
	onSubmit,
	onApprove,
	onSendBack,
	uploaderAge,
	onDiscard,
	onReanalyse,
	isBusy,
	mode = 'file',
	isSendingBack,
}: Props) {
	const { proposal, decision, destination } = item
	const friend = mode === 'submit'
	const reviewing = mode === 'review'
	const { t } = useTranslate()
	const [sendingBack, setSendingBack] = useState(false)
	// the same file as one in the library: the server refuses it from somebody whose books need
	// approval, so there is nothing to send. The owner's own filing allows it ("Add anyway")
	const sameFile =
		friend &&
		item.state !== IntakeState.Failed &&
		(proposal?.duplicates ?? []).some((duplicate) => duplicate.reason === 'SAME_FILE')
	const sentBack = isSentBack(item)
	const meta = decision?.metadata
	const isSeriesItem = item.kind === IntakeKind.Series
	const seriesKind = decision?.seriesKind ?? null
	// a manga or light novel library holds series of its own kind only: nothing to choose
	const libraryHoldsOneKind = typedSeriesKind(item.libraryType) !== null
	const filedAsSeries = isSeriesItem || !!seriesKind
	const attention = needsAttention(item)
	const ready = isReady(item)
	// a book that couldn't be read has nothing to add yet: its one button reads it again
	const failed = item.state === IntakeState.Failed && !reviewing
	const review = (input: IntakeReviewInput) => onReview(item.id, input)

	const tagsReviewed = useTagReviewStore((store) => !!store.reviewed[item.id])
	const markReviewed = useTagReviewStore((store) => store.markReviewed)
	const chosenGenres = useMemo(() => effectiveGenres(item, tagsReviewed), [item, tagsReviewed])
	const chips = useMemo(() => genreChips(item, chosenGenres), [item, chosenGenres])
	const setGenres = (genres: string[]) => {
		markReviewed(item.id)
		review({ genres })
	}
	const firstFormat = item.files[0]?.format
	const title = (filedAsSeries ? meta?.series : meta?.title) || item.name
	const singleFile = destination?.files.length === 1 ? destination.files[0] : undefined
	const authors = formatAuthors(meta?.authors ?? [])
	const byline = isSeriesItem
		? `${item.files.length} ${item.files.length === 1 ? 'volume' : 'volumes'}`
		: authors

	const folderCandidates = (filedAsSeries ? proposal?.seriesFolders : proposal?.authorFolders) ?? []

	return (
		<article
			className={cn(
				'gap-4 p-4 sm:p-5 flex flex-col rounded-xl border bg-card',
				attention || sentBack ? 'border-warning/50 ring-1 ring-warning/20' : 'border-border',
			)}
			aria-label={title}
		>
			<header className="gap-3 flex items-start">
				<CoverTile title={title} format={firstFormat} series={isSeriesItem} />
				<div className="min-w-0 gap-1 flex flex-1 flex-col">
					<div className="gap-2 flex items-start justify-between">
						<h3 className="text-base font-semibold leading-snug line-clamp-2 break-words">
							{title}
						</h3>
						<DropdownMenu
							align="end"
							trigger={
								<Button
									variant="ghost"
									size="icon"
									className="-mt-1 -mr-2 shrink-0"
									aria-label="More actions"
								>
									<EllipsisVertical className="h-4 w-4" />
								</Button>
							}
							groups={[
								{
									items: [
										{
											label: 'Read again',
											leftIcon: <RotateCw className="mr-2 h-4 w-4" />,
											onClick: () => onReanalyse(item.id),
										},
										{
											label: 'Look up online',
											leftIcon: <Globe className="mr-2 h-4 w-4" />,
											onClick: () => onReanalyse(item.id, true),
										},
										...(reviewing || !onDiscard
											? []
											: [
													{
														label: 'Discard',
														leftIcon: <Trash2 className="mr-2 h-4 w-4" />,
														onClick: () => onDiscard([item.id]),
													},
												]),
									],
								},
							]}
						/>
					</div>
					{byline ? (
						<p className="text-sm truncate text-muted-foreground">{byline}</p>
					) : (
						decision &&
						!filedAsSeries && <p className="text-sm text-muted-foreground">No author yet</p>
					)}
					{reviewing && <UploaderRow uploader={item.uploader} submittedAt={item.submittedAt} />}
					<div className="gap-x-2 gap-y-1 text-xs flex flex-wrap items-center text-muted-foreground">
						<IntakeStatusChip item={item} friend={friend} reviewing={reviewing} />
						{firstFormat && (
							<Badge variant="secondary" size="md">
								{formatLabel(firstFormat)}
							</Badge>
						)}
						{/* the library first, so a long file name can't push out where the book goes */}
						<span className="min-w-0 truncate" title={`${item.libraryName} · ${item.name}`}>
							{item.libraryName} · {item.name}
						</span>
					</div>
				</div>
			</header>

			<CardNotices
				item={item}
				friend={friend}
				reviewing={reviewing}
				sentBack={sentBack}
				uploaderAge={uploaderAge}
				isBusy={isBusy}
				onChangeLibrary={onChangeLibrary}
			/>

			{decision && (
				<>
					{/* what is asked of whoever looks: title, author, series, tags, description */}
					<div className="gap-3 grid grid-cols-6">
						{filedAsSeries ? (
							<>
								<ReviewField
									className={isSeriesItem ? 'col-span-6' : 'sm:col-span-5 col-span-4'}
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
										className="sm:col-span-1 col-span-2"
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
							</>
						) : (
							<>
								<ReviewField
									className="col-span-6"
									label="Title"
									value={meta?.title ?? ''}
									source={fieldSource(meta, 'title')}
									invalid={!meta?.title}
									onCommit={(value) => review({ title: value })}
									disabled={isBusy}
								/>
								<ReviewField
									className="sm:col-span-3 col-span-6"
									label="Authors"
									hint="Separate several authors with ;"
									value={authors}
									source={fieldSource(meta, 'authors')}
									invalid={!meta?.authors.length}
									placeholder="e.g. Umberto Eco"
									onCommit={(value) => review({ authors: parseAuthors(value) })}
									disabled={isBusy}
								/>
								<ReviewField
									className="sm:col-span-2 col-span-4"
									label="Series"
									value={meta?.series ?? ''}
									source={fieldSource(meta, 'series')}
									placeholder="None"
									onCommit={(series) => review({ series })}
									disabled={isBusy}
								/>
								<ReviewField
									className="sm:col-span-1 col-span-2"
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
							</>
						)}
					</div>

					{!filedAsSeries || chips.length > 0 ? (
						<section className="gap-2 flex flex-col" aria-label="Tags">
							<div className="gap-2 flex items-center">
								<span className="text-xs text-muted-foreground">Genres</span>
								<span className="text-xs text-muted-foreground">
									{chosenGenres.length ? `${chosenGenres.length} chosen` : 'none chosen'}
								</span>
							</div>
							<TagPicker
								libraryId={item.libraryId}
								chips={chips}
								chosen={chosenGenres}
								onChange={setGenres}
								disabled={isBusy}
							/>
						</section>
					) : null}

					<ReviewTextArea
						label="Description"
						rows={2}
						value={meta?.description ?? ''}
						source={fieldSource(meta, 'description')}
						placeholder="What the book is about"
						hint={friend ? 'Plain text' : undefined}
						maxLength={friend ? DESCRIPTION_MAX : undefined}
						onCommit={(description) => review({ description })}
						disabled={isBusy}
					/>

					{/* where it goes: the approver's to choose, so only they see it */}
					{!friend && (
						<div className="gap-2 p-3 flex items-start rounded-lg bg-muted/50">
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
					)}

					{/* opens by itself when what blocks the book is fixed in here (the folder, the volumes) */}
					<MoreDetails
						open={item.problems.length > 0}
						hint={friend ? 'language, year, publisher' : 'folder, language, year, age rating'}
					>
						{!friend && (
							<div className="gap-2 flex flex-wrap items-center">
								<FolderPicker
									libraryId={item.libraryId}
									seriesKind={seriesKind}
									candidates={folderCandidates}
									value={decision.folder}
									onSelect={(folder) => review({ folder })}
									disabled={isBusy}
								/>
								{!libraryHoldsOneKind && (
									<SeriesKindSwitch
										isSeriesItem={isSeriesItem}
										value={seriesKind}
										disabled={isBusy}
										onChange={(kind) =>
											review(kind ? { seriesKind: kind } : { clearSeriesKind: true })
										}
									/>
								)}
							</div>
						)}
						{libraries.length > 1 && (
							<LibrarySwitch
								value={item.libraryId}
								libraries={libraries}
								disabled={isBusy}
								onChange={(libraryId) => onChangeLibrary(item.id, libraryId)}
							/>
						)}
						<div className="gap-3 sm:grid-cols-6 grid grid-cols-2">
							<ReviewField
								className="sm:col-span-2"
								label="Language"
								value={meta?.language ?? ''}
								source={fieldSource(meta, 'language')}
								placeholder="en"
								onCommit={(language) => review({ language })}
								disabled={isBusy}
							/>
							{!filedAsSeries && (
								<ReviewField
									className="sm:col-span-2"
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
							)}
							{!friend && (
								<AgeRatingField
									className="sm:col-span-2 gap-1 flex flex-col"
									value={meta?.ageRating}
									disabled={isBusy}
									onChange={(ageRating) => review({ ageRating })}
								/>
							)}
							<ReviewField
								className="sm:col-span-6 col-span-2"
								label="Publisher"
								value={meta?.publisher ?? ''}
								source={fieldSource(meta, 'publisher')}
								onCommit={(publisher) => review({ publisher })}
								disabled={isBusy}
							/>
						</div>
						{(isSeriesItem || item.files.length > 1) && (
							<VolumeList
								item={item}
								disabled={isBusy}
								onChange={(volumes) => review({ volumes })}
							/>
						)}
						<SimilarBooks
							item={item}
							disabled={isBusy}
							onLooksRight={() => setGenres(chosenGenres)}
						/>
					</MoreDetails>
				</>
			)}

			<BlockingNotices item={item} friend={friend} reviewing={reviewing} />

			<footer className="gap-2 sm:flex-row sm:items-center sm:justify-end flex flex-col-reverse">
				{sameFile ? (
					<>
						<span
							className="text-sm sm:mr-auto text-muted-foreground"
							data-testid="intake-same-file"
						>
							{t('intakeScene.alreadyInLibrary')}
						</span>
						{onDiscard && (
							<Button onClick={() => onDiscard([item.id])} disabled={isBusy}>
								{t('intakeScene.removeFromList')}
							</Button>
						)}
					</>
				) : (
					<>
						{reviewing ? (
							<Button
								variant="outline"
								onClick={() => setSendingBack(true)}
								disabled={isBusy || !onSendBack}
							>
								Send back…
							</Button>
						) : (
							onDiscard && (
								<Button variant="ghost" onClick={() => onDiscard([item.id])} disabled={isBusy}>
									Discard
								</Button>
							)
						)}
						<Button
							className="sm:min-w-40"
							onClick={() => {
								if (failed) onReanalyse(item.id)
								else if (reviewing) onApprove?.(item, chosenGenres)
								else if (friend) onSubmit?.(item, chosenGenres)
								else onAccept?.(item, chosenGenres)
							}}
							disabled={
								failed
									? isBusy
									: isBusy ||
										!decision ||
										item.problems.length > 0 ||
										item.state === IntakeState.Failed
							}
							title={failed ? undefined : item.problems[0]}
						>
							{failed && <RotateCw />}
							{primaryLabel({ attention, friend, ready, reviewing, sentBack, state: item.state })}
						</Button>
					</>
				)}
			</footer>

			{reviewing && onSendBack && (
				<SendBackDialog
					open={sendingBack}
					title={title}
					to={item.uploader?.username}
					isPending={isSendingBack}
					onClose={() => setSendingBack(false)}
					onConfirm={(note) => onSendBack(item.id, note, () => setSendingBack(false))}
				/>
			)}
		</article>
	)
}

/** What the main button says: what it will do with the book, for whoever is looking */
export function primaryLabel({
	friend,
	reviewing,
	sentBack,
	state,
	ready,
	attention,
}: {
	friend: boolean
	reviewing: boolean
	sentBack: boolean
	state: IntakeState
	ready: boolean
	attention: boolean
}) {
	if (reviewing) return 'Approve'
	if (state === IntakeState.Failed) return 'Try again'
	if (friend) {
		if (sentBack) return 'Send again'
		if (state === IntakeState.NeedsReview) return 'Looks right, send it'
		return ready && !attention ? 'Send for approval' : 'Send anyway'
	}
	if (state === IntakeState.NeedsReview) return 'Looks right, add it'
	return ready && !attention ? 'Add to library' : 'Add anyway'
}
