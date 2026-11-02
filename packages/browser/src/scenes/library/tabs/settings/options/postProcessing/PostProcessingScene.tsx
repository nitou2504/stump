import { useGraphQL, useGraphQLMutation } from '@stump/client'
import {
	Alert,
	AlertDescription,
	AlertTitle,
	Button,
	ConfirmationModal,
	DropdownMenu,
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
			quarantineDir
			pdfQueueDir
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
	date ? `on for books added since ${new Date(date).toLocaleDateString()}` : null

type RunArgs = { grey16?: boolean; pdfOcr?: boolean; pdfShrink?: boolean; dryRun?: boolean }

export default function PostProcessingScene() {
	const {
		library: { id: libraryId },
	} = useLibraryManagement()
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
			toast.success('Post-processing settings saved')
			invalidate()
		},
		onError: () => toast.error('Failed to save the post-processing settings'),
	})
	const { mutate: run, isPending: isRunning } = useGraphQLMutation(runMutation, {
		onSuccess: () => {
			toast.success('Post-processing started: follow it below or in Settings → Jobs')
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
	const noQuarantine = !settings.quarantineDir
	const worker = settings.pdfWorker
	const lastSeen = worker.lastSeen
		? formatDistanceToNow(new Date(worker.lastSeen), { addSuffix: true })
		: null

	return (
		<div className="gap-10 flex flex-col">
			{noQuarantine && (
				<Alert variant="warning">
					<AlertTriangle />
					<AlertTitle>No quarantine folder</AlertTitle>
					<AlertDescription>
						Processed files replace the originals, which are kept in a quarantine folder and never
						deleted. Set <code>STUMP_QUARANTINE_DIR</code> (or <code>quarantine_dir</code> in
						Stump.toml) to a folder mounted in the container; until then nothing is replaced and
						only the 16-grey estimate works.
					</AlertDescription>
				</Alert>
			)}

			<NewCard
				label="Manga: 16 grey levels"
				description="Grey PNG pages of CBZ files become 16-grey 4-bit PNGs, what an e-ink screen shows. Colour pages and JPEG pages stay as they are; a volume is replaced only when it saves at least 1 MB and 5 %."
			>
				<NewCard.Row
					label="Run on"
					description={since(config.grey16Since) ?? 'Off: run it from here when you want'}
				>
					<NativeSelect
						className="w-40"
						options={MODE_OPTIONS}
						value={config.grey16}
						onChange={(e) => save({ grey16: e.target.value as PostProcessMode })}
					/>
				</NewCard.Row>
				<NewCard.Row
					label="Leave out"
					description="One path part per line, matched inside the library (e.g. /Berserk/: its pages are already compact JPEGs)"
				>
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
						count(PostProcessKind.Grey_16, PostProcessStatus.Done) > 0
							? `${count(PostProcessKind.Grey_16, PostProcessStatus.Done)} volumes converted so far, ${formatBytes(settings.bytesSaved)} saved in this library`
							: 'Estimate first: it converts a few pages of each volume and changes nothing'
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
						<Button
							variant="outline"
							disabled={isRunning || noQuarantine}
							onClick={() => setConfirmConvert(true)}
						>
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
						<AlertTitle>PDF worker container not running</AlertTitle>
						<AlertDescription>
							{lastSeen ? `Its last heartbeat was ${lastSeen}. ` : 'It has never checked in. '}
							Start the <code>stump-pdf-worker</code> service of the docker compose file (see{' '}
							<code>docker/compose.fork-postproc.example.yml</code>). Until it runs, PDFs are not
							queued from here, and PDFs that new books queue wait for it.
						</AlertDescription>
					</Alert>
				)}
				<NewCard
					label="Scanned PDFs"
					description="OCR for scans whose text layer is missing or poor, and size reduction when it saves at least 15 % and 2 MB without visible loss. Born-digital PDFs are left alone. These run in the PDF worker container next to Stump (OCRmyPDF and book-curator's recipe), which uses almost no memory while nothing is queued."
				>
					<NewCard.Row
						label="Worker"
						description={
							worker.alive
								? `The stump-pdf-worker container${worker.version ? ` (${worker.version})` : ''}, last heartbeat ${lastSeen}`
								: 'The stump-pdf-worker container is not running'
						}
						value={worker.alive ? (worker.state === 'busy' ? 'Working' : 'Idle') : 'Offline'}
					/>
					<NewCard.Row label="OCR" description={since(config.pdfOcrSince) ?? 'Off'}>
						<NativeSelect
							className="w-40"
							options={MODE_OPTIONS}
							value={config.pdfOcr}
							onChange={(e) => save({ pdfOcr: e.target.value as PostProcessMode })}
						/>
					</NewCard.Row>
					<NewCard.Row label="Shrink" description={since(config.pdfShrinkSince) ?? 'Off'}>
						<NativeSelect
							className="w-40"
							options={MODE_OPTIONS}
							value={config.pdfShrink}
							onChange={(e) => save({ pdfShrink: e.target.value as PostProcessMode })}
						/>
					</NewCard.Row>
					<NewCard.Row
						label="Queue this library now"
						description={
							<>
								Requests go to <code className="break-all">{settings.pdfQueueDir}</code>; the PDF
								worker picks them up and Stump swaps the results in.
							</>
						}
					>
						<DropdownMenu
							trigger={
								<Button variant="outline" disabled={isRunning || noQuarantine || !worker.alive}>
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
				description="Every CBZ that is not left out and not converted yet is reduced to 16 grey levels, one volume at a time at low priority. Each original is copied to the quarantine folder before it is replaced."
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
