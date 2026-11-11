import { useGraphQL } from '@stump/client'
import { Alert, AlertDescription, AlertTitle, Button, cn, ProgressBar } from '@stump/components'
import {
	CheckCircle2,
	CircleAlert,
	ClipboardCheck,
	Hourglass,
	Library,
	Loader2,
	MessageSquareWarning,
	Send,
	Upload,
} from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'
import { Helmet } from 'react-helmet'
import { useSearchParams } from 'react-router'

import { SceneContainer } from '@/components/container'
import { CollectionSceneHeader } from '@/components/sharedLayout'

import AcceptAllSheet from './AcceptAllSheet'
import { useIntakeActions, useIntakeInbox, useIntakeLimits, useIntakeUploader } from './hooks'
import { AnalysingRow, FiledRow } from './InboxRows'
import IntakeDropZone from './IntakeDropZone'
import IntakeItemCard, { type IntakeCardMode } from './IntakeItemCard'
import { intakeLibrariesQuery, type IntakeLimitsData } from './queries'
import { acceptableIds, formatBytes, groupInbox } from './reviewState'
import { useTagReviewStore } from './tagReviewStore'
import WaitingRow from './WaitingRow'

const FILED_SHOWN = 10

/**
 * "Add books": upload books into your own inbox, check what the server proposes (author
 * folder, metadata, tags) and add them to the library. Somebody who may not file books
 * themselves sends them for approval instead: the people who run the library look at them first.
 *
 * The page reads top to bottom as the books move: what is being read, what came back with a
 * note, what needs a look, what is ready, then what is with the approver and what is in.
 */
export default function IntakeScene() {
	const [params, setParams] = useSearchParams()
	const { data: librariesData } = useGraphQL(intakeLibrariesQuery, ['intakeLibraries'])
	const libraries = useMemo(() => librariesData?.intakeLibraries ?? [], [librariesData])

	const requested = params.get('library')
	const libraryId =
		libraries.find((library) => library.id === requested)?.id ?? libraries[0]?.id ?? undefined

	const { items, isLoading, data: inboxData } = useIntakeInbox()
	const { limits } = useIntakeLimits()
	// what the cards show and do depends on who is looking, which the limits say; until they
	// arrive nothing is shown that could be wrong for the viewer
	const mode: IntakeCardMode | undefined = limits
		? limits.canFileDirectly
			? 'file'
			: 'submit'
		: undefined
	const friend = mode === 'submit'
	// forget the tag choices of items that left the inbox
	const keepOnlyReviewed = useTagReviewStore((store) => store.keepOnly)
	useEffect(() => {
		if (inboxData) keepOnlyReviewed(items.map((item) => item.id))
	}, [inboxData, items, keepOnlyReviewed])
	const actions = useIntakeActions()
	const uploader = useIntakeUploader(libraryId, limits)
	const groups = useMemo(() => groupInbox(items), [items])
	const readyIds = useMemo(() => acceptableIds(items), [items])
	const readyItems = useMemo(
		() => items.filter((item) => readyIds.includes(item.id)),
		[items, readyIds],
	)
	const [confirmingAll, setConfirmingAll] = useState(false)
	const [showAllFiled, setShowAllFiled] = useState(false)
	const now = useNow()

	// keep the chosen library in the URL so the page can be bookmarked / come back to
	useEffect(() => {
		if (libraryId && requested !== libraryId) {
			setParams(
				(prev) => {
					prev.set('library', libraryId)
					return prev
				},
				{ replace: true },
			)
		}
	}, [libraryId, requested, setParams])

	const busy =
		actions.isReviewing ||
		actions.isAccepting ||
		actions.isChangingLibrary ||
		actions.isSubmitting ||
		actions.isWithdrawing
	const cardProps = {
		isBusy: busy,
		libraries,
		mode,
		onAccept: actions.acceptItem,
		onChangeLibrary: actions.changeLibrary,
		onDiscard: actions.discard,
		onReanalyse: actions.reanalyse,
		onReview: actions.reviewItem,
		onSubmit: actions.submitItem,
	}
	const uploadsClosed = !!limits && !limits.uploadsOpen

	const filed = showAllFiled ? groups.filed : groups.filed.slice(0, FILED_SHOWN)
	// books that are still on their way: the drop zone steps aside for them
	const inFlight =
		groups.analysing.length +
		groups.toReview.length +
		groups.ready.length +
		groups.sentBack.length +
		groups.waiting.length +
		groups.filing.length

	return (
		<>
			<Helmet>
				<title>Stump | Add books</title>
			</Helmet>

			<CollectionSceneHeader
				title="Add books"
				stats={[
					{
						key: 'review',
						icon: ClipboardCheck,
						value: groups.toReview.length,
						suffix: ' need your details',
					},
					{ key: 'ready', icon: CheckCircle2, value: groups.ready.length, suffix: ' ready' },
					...(friend && groups.waiting.length
						? [
								{
									key: 'waiting',
									icon: Hourglass,
									value: groups.waiting.length,
									suffix: ' waiting',
								},
							]
						: []),
				]}
			/>

			<SceneContainer className="gap-6 max-w-5xl mx-auto">
				<div className="gap-3 flex flex-col">
					{friend && (
						<p className="text-sm text-muted-foreground">
							Whoever runs this library looks over your books before they join it.
						</p>
					)}
					{limits?.quotaApplies && <InboxUsage limits={limits} />}
					{uploadsClosed && (
						<Alert variant="warning">
							<CircleAlert />
							<AlertTitle>Uploads are closed</AlertTitle>
							<AlertDescription>
								{limits?.closedReason || 'You can’t add books right now.'}
							</AlertDescription>
						</Alert>
					)}
					<IntakeDropZone
						onFiles={uploader.add}
						entries={uploader.entries}
						onClearFinished={uploader.clearFinished}
						disabled={!libraryId || uploadsClosed}
						compact={inFlight > 0}
						// a zip of several books is for who files them; until we know who is looking, none
						acceptsZip={mode === 'file'}
						libraries={libraries}
						libraryId={libraryId}
						onLibraryChange={(id) =>
							setParams(
								(prev) => {
									prev.set('library', id)
									return prev
								},
								{ replace: true },
							)
						}
					/>
					{!libraryId && !isLoading && (
						<p className="text-sm text-muted-foreground">You can’t add books to any library yet.</p>
					)}
				</div>

				{groups.analysing.length > 0 && (
					<Section title="Being read" icon={Loader2} count={groups.analysing.length}>
						<RowList>
							{groups.analysing.map((item) => (
								<AnalysingRow key={item.id} item={item} />
							))}
						</RowList>
					</Section>
				)}

				{mode && groups.sentBack.length > 0 && (
					<Section
						title="Sent back"
						icon={MessageSquareWarning}
						count={groups.sentBack.length}
						description="Change what they ask for, then send them again"
					>
						<div className="gap-4 flex flex-col">
							{groups.sentBack.map((item) => (
								<IntakeItemCard key={item.id} item={item} {...cardProps} />
							))}
						</div>
					</Section>
				)}

				{mode && groups.toReview.length > 0 && (
					<Section title="Needs your details" icon={ClipboardCheck} count={groups.toReview.length}>
						<div className="gap-4 flex flex-col">
							{groups.toReview.map((item) => (
								<IntakeItemCard key={item.id} item={item} {...cardProps} />
							))}
						</div>
					</Section>
				)}

				{mode && groups.ready.length > 0 && (
					<Section
						title={friend ? 'Ready to send' : 'Ready to add'}
						icon={friend ? Send : CheckCircle2}
						count={groups.ready.length}
						action={
							readyIds.length > 0 && (
								<Button size="sm" onClick={() => setConfirmingAll(true)} disabled={busy}>
									{friend ? 'Send all ready' : 'Add all ready'} ({readyIds.length})
								</Button>
							)
						}
					>
						<div className="gap-4 flex flex-col">
							{groups.ready.map((item) => (
								<IntakeItemCard key={item.id} item={item} {...cardProps} />
							))}
						</div>
					</Section>
				)}

				{groups.waiting.length > 0 && (
					<Section
						title="Waiting for approval"
						icon={Hourglass}
						count={groups.waiting.length}
						description="You can take a book back to change it until it is approved"
					>
						<RowList>
							{groups.waiting.map((item) => (
								<WaitingRow
									key={item.id}
									item={item}
									onWithdraw={actions.withdraw}
									disabled={busy}
								/>
							))}
						</RowList>
					</Section>
				)}

				{groups.filing.length > 0 && (
					<Section title="Being added" icon={Loader2} count={groups.filing.length}>
						<RowList>
							{groups.filing.map((item) => (
								<WaitingRow
									key={item.id}
									item={item}
									onWithdraw={actions.withdraw}
									disabled={busy}
								/>
							))}
						</RowList>
					</Section>
				)}

				{/* the last step differs for who sends books for approval: wait until we know who this is */}
				{!isLoading && mode && inFlight === 0 && groups.filed.length === 0 && (
					<EmptyInbox friend={friend} />
				)}

				{groups.filed.length > 0 && (
					<Section title="Added" icon={CheckCircle2} count={groups.filed.length}>
						<RowList>
							{filed.map((item) => (
								<FiledRow key={item.id} item={item} now={now} />
							))}
						</RowList>
						{groups.filed.length > FILED_SHOWN && (
							<Button variant="ghost" size="sm" onClick={() => setShowAllFiled((v) => !v)}>
								{showAllFiled ? 'Show fewer' : `Show all ${groups.filed.length}`}
							</Button>
						)}
					</Section>
				)}
			</SceneContainer>

			<AcceptAllSheet
				items={readyItems}
				open={confirmingAll}
				onClose={() => setConfirmingAll(false)}
				isPending={actions.isAccepting || actions.isSubmitting}
				verb={friend ? 'send' : 'add'}
				onConfirm={(entries) => {
					if (friend) actions.submitWithGenres(entries)
					else actions.acceptWithGenres(entries)
					setConfirmingAll(false)
				}}
			/>
		</>
	)
}

function Section({
	title,
	icon: Icon,
	count,
	description,
	action,
	children,
}: {
	title: string
	icon: React.ComponentType<{ className?: string }>
	count: number
	description?: string
	action?: React.ReactNode
	children: React.ReactNode
}) {
	return (
		<section className="gap-3 flex flex-col" aria-label={title}>
			<div className="gap-3 flex flex-wrap items-center justify-between">
				<div className="min-w-0">
					<h2 className="gap-2 text-sm font-semibold flex items-center">
						<Icon className="h-4 w-4 text-muted-foreground" aria-hidden />
						{title}
						<span className="px-2 text-xs font-medium rounded-full bg-muted py-px text-muted-foreground tabular-nums">
							{count}
						</span>
					</h2>
					{description && <p className="mt-0.5 text-xs text-muted-foreground">{description}</p>}
				</div>
				{action}
			</div>
			{children}
		</section>
	)
}

/** Books that are not cards: a row each, in one rounded list */
function RowList({ children }: { children: React.ReactNode }) {
	return (
		<ul className="divide-y divide-border rounded-xl border border-border bg-card">{children}</ul>
	)
}

/** The current time, refreshed every 30 s (0 until mounted) */
function useNow() {
	const [now, setNow] = useState(0)
	useEffect(() => {
		setNow(Date.now())
		const timer = setInterval(() => setNow(Date.now()), 30_000)
		return () => clearInterval(timer)
	}, [])
	return now
}

/** Nothing in the inbox: what happens to a book, in three steps */
function EmptyInbox({ friend }: { friend: boolean }) {
	const steps = [
		{ icon: Upload, label: 'Drop your books' },
		{ icon: ClipboardCheck, label: 'Check the details' },
		friend
			? { icon: Send, label: 'Send them for approval' }
			: { icon: Library, label: 'Add them to the library' },
	]
	return (
		<div className="gap-4 py-6 flex flex-col items-center text-center">
			<p className="text-sm text-muted-foreground">Your inbox is empty.</p>
			<ol
				className="gap-x-6 gap-y-3 text-sm sm:flex-row flex flex-col items-center text-muted-foreground"
				aria-label="How adding books works"
			>
				{steps.map(({ icon: Icon, label }) => (
					<li key={label} className="gap-2 flex items-center">
						<span className="size-7 grid place-items-center rounded-full bg-muted">
							<Icon className="h-3.5 w-3.5" aria-hidden />
						</span>
						{label}
					</li>
				))}
			</ol>
		</div>
	)
}

/** How much of their inbox somebody who needs approval has used: room, and books */
function InboxUsage({ limits }: { limits: IntakeLimitsData }) {
	const room =
		limits.quotaBytes > 0 ? Math.min(100, (limits.usedBytes * 100) / limits.quotaBytes) : 0
	const full = limits.usedBytes >= limits.quotaBytes || limits.openItems >= limits.maxItems
	return (
		<div className="gap-1.5 text-xs flex flex-col text-muted-foreground" aria-label="Your inbox">
			<div className="gap-x-3 flex flex-wrap justify-between">
				<span>
					Your inbox: {formatBytes(limits.usedBytes)} of {formatBytes(limits.quotaBytes)}
				</span>
				<span className={cn(full && 'text-warning')}>
					{limits.openItems} of {limits.maxItems} books
				</span>
			</div>
			<ProgressBar value={Math.round(room)} size="sm" />
		</div>
	)
}
