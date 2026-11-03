import { useGraphQL, useGraphQLMutation } from '@stump/client'
import {
	Alert,
	AlertDescription,
	AlertTitle,
	Button,
	ConfirmationModal,
	DropdownMenu,
	Link,
	NativeSelect,
	NewCard,
	RawTextArea,
	Text,
} from '@stump/components'
import { graphql, PostProcessKind, PostProcessMode, PostProcessStatus } from '@stump/graphql'
import { useQueryClient } from '@tanstack/react-query'
import { formatDistanceToNow } from 'date-fns'
import { AlertTriangle, ChevronDown, FileScan, Gauge, Shrink } from 'lucide-react'
import { useCallback, useEffect, useState } from 'react'
import { toast } from 'sonner'

import PostProcessingTasks, { formatBytes } from '@/components/postProcessing/PostProcessingTasks'
import { usePaths } from '@/paths'

import { useLibraryManagement } from '../../context'

const query = graphql(`
	query LibraryPostProcessing($libraryId: ID!) {
		libraryPostProcessing(libraryId: $libraryId) {
			config {
				grey16
				grey16Exclude
				pdfOcr
				pdfShrink
				grey16Since
				pdfOcrSince
				pdfShrinkSince
			}
			pdfWorker {
				alive
				state
				version
				lastSeen
			}
			counts {
				kind
				status
				count
			}
			bytesSaved
		}
	}
`)

const updateMutation = graphql(`
	mutation UpdateLibraryPostProcessing($libraryId: ID!, $input: PostProcessingConfigInput!) {
		updateLibraryPostProcessing(libraryId: $libraryId, input: $input) {
			bytesSaved
		}
	}
`)

const runMutation = graphql(`
	mutation RunPostProcessing(
		$libraryId: ID!
		$grey16: Boolean!
		$pdfOcr: Boolean!
		$pdfShrink: Boolean!
		$dryRun: Boolean!
	) {
		runPostProcessing(
			libraryId: $libraryId
			grey16: $grey16
			pdfOcr: $pdfOcr
			pdfShrink: $pdfShrink
			dryRun: $dryRun
		)
	}
`)

const MODE_OPTIONS = [
	{ label: 'Off', value: PostProcessMode.Off },
	{ label: 'New books', value: PostProcessMode.New },
]

const since = (date?: string | null) =>
	date ? `Books added since ${new Date(date).toLocaleDateString()}` : undefined

type RunArgs = { grey16?: boolean; pdfOcr?: boolean; pdfShrink?: boolean; dryRun?: boolean }

export default function PostProcessingScene() {
	const {
		library: { id: libraryId },
	} = useLibraryManagement()
	const paths = usePaths()
	const client = useQueryClient()
	// refreshed every 30 s so the PDF worker's state (its heartbeat) stays current
	const { data } = useGraphQL(
		query,
		['libraryPostProcessing', libraryId],
		{ libraryId },
		{ refetchInterval: 30_000 },
	)
	const settings = data?.libraryPostProcessing
	const config = settings?.config

	const [exclude, setExclude] = useState('')
	useEffect(() => {
		if (config) setExclude(config.grey16Exclude.join('\n'))
	}, [config])
	const [confirmConvert, setConfirmConvert] = useState(false)

	const invalidate = useCallback(() => {
		client.invalidateQueries({ queryKey: ['libraryPostProcessing', libraryId] })
		client.invalidateQueries({ queryKey: ['postProcessingTasks'] })
	}, [client, libraryId])

	const { mutate: update } = useGraphQLMutation(updateMutation, {
		onSuccess: () => {
			toast.success('Saved')
			invalidate()
		},
		onError: () => toast.error('Failed to save'),
	})
	const { mutate: run, isPending: isRunning } = useGraphQLMutation(runMutation, {
		onSuccess: () => {
			toast.success('Post-processing started')
			setTimeout(invalidate, 1500)
		},
		onError: () => toast.error('Failed to start post-processing'),
	})

	const save = useCallback(
		(
			patch: Partial<{
				grey16: PostProcessMode
				pdfOcr: PostProcessMode
				pdfShrink: PostProcessMode
			}>,
		) => {
			if (!config) return
			update({
				libraryId,
				input: {
					grey16: patch.grey16 ?? config.grey16,
					grey16Exclude: exclude
						.split('\n')
						.map((line) => line.trim())
						.filter(Boolean),
					pdfOcr: patch.pdfOcr ?? config.pdfOcr,
					pdfShrink: patch.pdfShrink ?? config.pdfShrink,
				},
			})
		},
		[config, exclude, libraryId, update],
	)

	const start = useCallback(
		({ grey16 = false, pdfOcr = false, pdfShrink = false, dryRun = false }: RunArgs) =>
			run({ libraryId, grey16, pdfOcr, pdfShrink, dryRun }),
		[libraryId, run],
	)

	if (!settings || !config) return null

	const count = (kind: PostProcessKind, status: PostProcessStatus) =>
		settings.counts.find((c) => c.kind === kind && c.status === status)?.count ?? 0
	const worker = settings.pdfWorker
	const lastSeen = worker.lastSeen
		? formatDistanceToNow(new Date(worker.lastSeen), { addSuffix: true })
		: null
	const converted = count(PostProcessKind.Grey_16, PostProcessStatus.Done)

	return (
		<div className="gap-10 flex flex-col">
			<p className="text-sm max-w-prose text-muted-foreground">
				Originals wait in{' '}
				<Link to={paths.deletedBooks()} className="underline">
					Deleted books
				</Link>
				, where you can put them back.
			</p>

			<NewCard
				label="Manga: 16 greys"
				description="Grey pages reduced to the 16 levels e-ink shows."
			>
				<NewCard.Row label="Run on" description={since(config.grey16Since)}>
					<NativeSelect
						className="w-40"
						options={MODE_OPTIONS}
						value={config.grey16}
						onChange={(e) => save({ grey16: e.target.value as PostProcessMode })}
					/>
				</NewCard.Row>
				<NewCard.Row label="Skip folders" description="One per line, like /Berserk/">
					<RawTextArea
						className="sm:w-64 text-sm min-h-[4.5rem] w-full"
						value={exclude}
						onChange={(e) => setExclude(e.target.value)}
						onBlur={() => {
							if (exclude.trim() !== config.grey16Exclude.join('\n').trim()) save({})
						}}
					/>
				</NewCard.Row>
				<NewCard.Row
					label="This library now"
					description={
						converted > 0
							? `${converted === 1 ? '1 volume' : `${converted} volumes`} converted, ${formatBytes(settings.bytesSaved)} saved`
							: 'An estimate changes nothing'
					}
				>
					<div className="gap-2 flex flex-wrap">
						<Button
							variant="outline"
							disabled={isRunning}
							onClick={() => start({ grey16: true, dryRun: true })}
						>
							<Gauge className="mr-2 h-4 w-4" />
							Estimate savings
						</Button>
						<Button variant="outline" disabled={isRunning} onClick={() => setConfirmConvert(true)}>
							<Shrink className="mr-2 h-4 w-4" />
							Convert now
						</Button>
					</div>
				</NewCard.Row>
			</NewCard>

			<div className="gap-3 flex flex-col">
				{!worker.alive && (
					<Alert variant="warning">
						<AlertTriangle />
						<AlertTitle>PDF worker offline</AlertTitle>
						<AlertDescription>
							{lastSeen ? `Last seen ${lastSeen}. ` : ''}Queued PDFs wait until it runs.
						</AlertDescription>
					</Alert>
				)}
				<NewCard
					label="Scanned PDFs"
					description="OCR and shrink scans. Other PDFs are left alone."
				>
					<NewCard.Row
						label="Worker"
						description={
							worker.alive
								? `Last seen ${lastSeen}${worker.version ? ` · ${worker.version}` : ''}`
								: undefined
						}
						value={worker.alive ? (worker.state === 'busy' ? 'Working' : 'Idle') : 'Offline'}
					/>
					<NewCard.Row label="OCR" description={since(config.pdfOcrSince)}>
						<NativeSelect
							className="w-40"
							options={MODE_OPTIONS}
							value={config.pdfOcr}
							onChange={(e) => save({ pdfOcr: e.target.value as PostProcessMode })}
						/>
					</NewCard.Row>
					<NewCard.Row label="Shrink" description={since(config.pdfShrinkSince)}>
						<NativeSelect
							className="w-40"
							options={MODE_OPTIONS}
							value={config.pdfShrink}
							onChange={(e) => save({ pdfShrink: e.target.value as PostProcessMode })}
						/>
					</NewCard.Row>
					<NewCard.Row label="This library now">
						<DropdownMenu
							trigger={
								<Button variant="outline" disabled={isRunning || !worker.alive}>
									<FileScan className="mr-2 h-4 w-4" />
									Queue PDFs
									<ChevronDown className="ml-2 h-4 w-4" />
								</Button>
							}
							groups={[
								{
									items: [
										{
											label: 'OCR and shrink',
											onClick: () => start({ pdfOcr: true, pdfShrink: true }),
										},
										{ label: 'OCR only', onClick: () => start({ pdfOcr: true }) },
										{ label: 'Shrink only', onClick: () => start({ pdfShrink: true }) },
									],
								},
							]}
							align="end"
						/>
					</NewCard.Row>
				</NewCard>
			</div>

			<div className="gap-2 flex flex-col">
				<Text className="font-medium">Recent tasks</Text>
				<PostProcessingTasks libraryId={libraryId} />
			</div>

			<ConfirmationModal
				title="Convert this library's manga now?"
				description="Originals wait in Deleted books, where you can put them back."
				isOpen={confirmConvert}
				onClose={() => setConfirmConvert(false)}
				onConfirm={() => {
					setConfirmConvert(false)
					start({ grey16: true })
				}}
				confirmText="Convert"
			/>
		</div>
	)
}
