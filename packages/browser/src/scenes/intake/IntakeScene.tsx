import { useGraphQL } from '@stump/client'
import { Button, cn, Link, NativeSelect } from '@stump/components'
import { IntakeKind } from '@stump/graphql'
import { CheckCircle2, ClipboardCheck, Hourglass, Inbox, Loader2 } from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'
import { Helmet } from 'react-helmet'
import { useSearchParams } from 'react-router'

import { SceneContainer } from '@/components/container'
import { CollectionSceneHeader } from '@/components/sharedLayout'
import { usePaths } from '@/paths'

import AcceptAllSheet from './AcceptAllSheet'
import { useIntakeActions, useIntakeInbox, useIntakeUploader } from './hooks'
import IntakeDropZone from './IntakeDropZone'
import IntakeItemCard, { FormatIcon } from './IntakeItemCard'
import { type IntakeItemData, intakeLibrariesQuery } from './queries'
import { acceptableIds, destinationPaths, groupInbox } from './reviewState'
import { useTagReviewStore } from './tagReviewStore'

const FILED_SHOWN = 10

/**
 * "Add books": upload books into your own inbox, check what the server proposes (author
 * folder, metadata, tags) and add them to the library.
 */
export default function IntakeScene() {
	const [params, setParams] = useSearchParams()
	const { data: librariesData } = useGraphQL(intakeLibrariesQuery, ['intakeLibraries'])
	const libraries = useMemo(() => librariesData?.intakeLibraries ?? [], [librariesData])

	const requested = params.get('library')
	const libraryId =
		libraries.find((library) => library.id === requested)?.id ?? libraries[0]?.id ?? undefined

	const { items, isLoading, data: inboxData } = useIntakeInbox()
	// forget the tag choices of items that left the inbox
	const keepOnlyReviewed = useTagReviewStore((store) => store.keepOnly)
	useEffect(() => {
		if (inboxData) keepOnlyReviewed(inboxData.intakeItems.map((item) => item.id))
	}, [inboxData, keepOnlyReviewed])
	const actions = useIntakeActions()
	const uploader = useIntakeUploader(libraryId)
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

	const busy = actions.isReviewing || actions.isAccepting
	const cardProps = {
		isBusy: busy,
		onAccept: actions.acceptItem,
		onDiscard: actions.discard,
		onReanalyse: actions.reanalyse,
		onReview: actions.reviewItem,
	}

	const filed = showAllFiled ? groups.filed : groups.filed.slice(0, FILED_SHOWN)

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
						suffix: ' to check',
					},
					{ key: 'ready', icon: CheckCircle2, value: groups.ready.length, suffix: ' ready' },
				]}
			/>

			<SceneContainer className="gap-6 max-w-5xl mx-auto">
				<div className="gap-3 flex flex-col">
					{libraries.length > 1 && (
						<label className="gap-2 text-sm flex items-center">
							<span className="text-muted-foreground">Add to</span>
							<NativeSelect
								size="sm"
								className="text-base md:text-sm w-auto"
								value={libraryId}
								options={libraries.map((library) => ({ label: library.name, value: library.id }))}
								onChange={(e) =>
									setParams(
										(prev) => {
											prev.set('library', e.target.value)
											return prev
										},
										{ replace: true },
									)
								}
							/>
						</label>
					)}
					<IntakeDropZone
						onFiles={uploader.add}
						entries={uploader.entries}
						onClearFinished={uploader.clearFinished}
						disabled={!libraryId}
					/>
					{!libraryId && !isLoading && (
						<p className="text-sm text-muted-foreground">You can’t add books to any library yet.</p>
					)}
				</div>

				{groups.analysing.length > 0 && (
					<Section title="Being analysed" icon={Hourglass} count={groups.analysing.length}>
						<ul className="divide-y divide-border rounded-lg border border-border">
							{groups.analysing.map((item) => (
								<li key={item.id} className="gap-3 px-3 py-2 flex items-center">
									<Loader2
										className="h-4 w-4 animate-spin shrink-0 text-muted-foreground"
										aria-hidden
									/>
									<span className="text-sm flex-1 truncate">{item.name}</span>
									<span className="text-xs text-muted-foreground">reading metadata…</span>
								</li>
							))}
						</ul>
					</Section>
				)}

				{groups.toReview.length > 0 && (
					<Section
						title="To check"
						icon={ClipboardCheck}
						count={groups.toReview.length}
						description="Something is missing or uncertain. Fix it, or confirm it looks right."
					>
						<div className="gap-4 flex flex-col">
							{groups.toReview.map((item) => (
								<IntakeItemCard key={item.id} item={item} {...cardProps} />
							))}
						</div>
					</Section>
				)}

				{groups.ready.length > 0 && (
					<Section
						title="Ready to add"
						icon={CheckCircle2}
						count={groups.ready.length}
						action={
							readyIds.length > 0 && (
								<Button size="sm" onClick={() => setConfirmingAll(true)} disabled={busy}>
									Add all ready ({readyIds.length})
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

				{!isLoading &&
					!groups.analysing.length &&
					!groups.toReview.length &&
					!groups.ready.length && (
						<div className="gap-2 py-10 flex flex-col items-center text-center text-muted-foreground">
							<Inbox className="h-8 w-8" aria-hidden />
							<p className="text-sm">
								Your inbox is empty. Uploaded books wait here until you add them.
							</p>
						</div>
					)}

				{groups.filed.length > 0 && (
					<Section title="Added" icon={CheckCircle2} count={groups.filed.length}>
						<ul className="divide-y divide-border rounded-lg border border-border">
							{filed.map((item) => (
								<FiledRow key={item.id} item={item} now={now} />
							))}
						</ul>
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
				isPending={actions.isAccepting}
				onConfirm={(entries) => {
					actions.acceptWithGenres(entries)
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
			<div className="gap-3 flex flex-wrap items-end justify-between">
				<div>
					<h2 className="gap-2 font-semibold flex items-center">
						<Icon className="h-4 w-4 text-muted-foreground" />
						{title}
						<span className="text-sm font-normal text-muted-foreground">{count}</span>
					</h2>
					{description && <p className="text-sm text-muted-foreground">{description}</p>}
				</div>
				{action}
			</div>
			{children}
		</section>
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

function FiledRow({ item, now }: { item: IntakeItemData; now: number }) {
	const paths = usePaths()
	const isSeries = item.kind === IntakeKind.Series
	const meta = item.decision?.metadata
	const title = (isSeries || item.decision?.seriesKind ? meta?.series : meta?.title) || item.name
	const where = destinationPaths(item)
	const first = item.mediaIds[0]
	const recentlyFiled =
		!now || (!!item.filedAt && now - new Date(item.filedAt).getTime() < 5 * 60 * 1000)

	return (
		<li className="gap-3 px-3 py-2.5 flex items-start">
			<FormatIcon
				format={item.files[0]?.format}
				series={isSeries}
				className="mt-0.5 h-4 w-4 shrink-0 text-muted-foreground"
			/>
			<div className="min-w-0 flex flex-1 flex-col">
				<span className="text-sm font-medium truncate">{title}</span>
				<span
					className={cn('font-mono text-xs break-all text-muted-foreground')}
					title={where.join('\n')}
				>
					{where.length > 1
						? `${item.destination?.relativeDir}/ (${where.length} files)`
						: where[0]}
				</span>
				{!!item.result?.notes.length && (
					<span className="text-xs text-warning">{item.result.notes.join(' · ')}</span>
				)}
			</div>
			{first ? (
				<Link to={paths.bookOverview(first)} className="text-sm shrink-0">
					Open
				</Link>
			) : recentlyFiled ? (
				<span className="gap-1 text-xs flex shrink-0 items-center text-muted-foreground">
					<Loader2 className="h-3 w-3 animate-spin" aria-hidden /> scanning
				</span>
			) : (
				<span className="text-xs shrink-0 text-muted-foreground">not scanned yet</span>
			)}
		</li>
	)
}
