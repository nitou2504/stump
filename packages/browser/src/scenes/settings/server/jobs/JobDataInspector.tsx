import { useGraphQLFetcher } from '@stump/client'
import { Card, Preformatted, Sheet, Text, usePrevious } from '@stump/components'
import {
	FragmentType,
	graphql,
	JobDataInspectorLogsQuery,
	JobTableQuery,
	useFragment,
} from '@stump/graphql'
import { Api } from '@stump/sdk'
import { QueryClient } from '@tanstack/react-query'

import { Table } from '@/components/table'
import { useTranslate } from '@/hooks/useTranslate'

import { columns } from '../logs/persisted-logs/PersistedLogsTable'

const fragment = graphql(`
	fragment JobDataInspector on CoreJobOutput {
		__typename
		... on LibraryScanOutput {
			totalFiles
			totalDirectories
			ignoredFiles
			skippedFiles
			ignoredDirectories
			createdMedia
			updatedMedia
			createdSeries
			updatedSeries
		}
		... on SeriesScanOutput {
			totalFiles
			ignoredFiles
			skippedFiles
			createdMedia
			updatedMedia
		}
		... on ThumbnailGenerationOutput {
			visitedFiles
			skippedFiles
			generatedThumbnails
			removedThumbnails
		}
		... on TrashPurgeOutput {
			purgedBooks
			missingBookFiles
			keptBooks
			purgedHeldFiles
			freedBytes
		}
		... on RescanBooksOutput {
			rescannedBooks
			updatedBooks
			missingBooks
		}
	}
`)

export type JobDataInspectorFragment = FragmentType<typeof fragment>

type Job = JobTableQuery['jobs']['nodes'][number]

const logsQuery = graphql(`
	query JobDataInspectorLogs($id: String!, $page: Int!, $pageSize: Int!) {
		logs(
			filter: { jobId: { eq: $id } }
			pagination: { offset: { page: $page, pageSize: $pageSize } }
		) {
			nodes {
				id
				level
				message
				timestamp
			}
		}
	}
`)

// The server hands out at most 200 log entries at a time (and refuses to give them all at once)
const JOB_LOGS_PAGE_SIZE = 200
// A job with more than this many entries (10,000) is cut, rather than read for ever
const JOB_LOGS_MAX_PAGES = 50

type JobLog = JobDataInspectorLogsQuery['logs']['nodes'][number]

/** All the log entries of a job: page after page, until one comes back that isn't full */
async function fetchJobLogs(sdk: Api, id: string): Promise<JobLog[]> {
	const logs: JobLog[] = []
	for (let page = 1; page <= JOB_LOGS_MAX_PAGES; page++) {
		const response = await sdk.execute(logsQuery, { id, page, pageSize: JOB_LOGS_PAGE_SIZE })
		logs.push(...response.logs.nodes)
		if (response.logs.nodes.length < JOB_LOGS_PAGE_SIZE) break
	}
	return logs
}

export function prefetchJobLogs(sdk: Api, client: QueryClient, jobId: string) {
	return client.prefetchQuery({
		queryKey: ['logsForJob', jobId],
		queryFn: () => fetchJobLogs(sdk, jobId),
	})
}

type Props = {
	job?: Job | null
	onClose: () => void
}

// TODO: pull in logs from this job, bumps complexity a bit but worth i think (even tho there is a see logs btn)

export default function JobDataInspector({ job, onClose }: Props) {
	const { t } = useTranslate()

	const inlineData = useFragment(fragment, job?.outputData)
	const fallback = usePrevious(inlineData)
	const displayedData = inlineData || fallback

	const { data } = useGraphQLFetcher(
		['logsForJob', job?.id],
		(sdk) => fetchJobLogs(sdk, job?.id ?? ''),
		{
			enabled: !!job?.id && job.logCount > 0,
		},
	)
	const jobLogs = data ?? []

	return (
		<Sheet
			open={!!job}
			onClose={onClose}
			title={t(getKey('title'))}
			description={t(getKey('description'))}
		>
			<div className="px-4 gap-y-8 flex flex-col">
				<Preformatted content={displayedData} />

				{jobLogs.length > 0 && (
					<div className="gap-y-4 flex flex-col">
						<Text className="font-semibold">{t(getKey('associatedLogs'))}</Text>
						<Card className="overflow-hidden">
							<Table
								sortable
								columns={columns.slice(0, -1)} //  we don't need job id
								data={jobLogs}
								fullWidth
								isZeroBasedPagination
								cellClassName="bg-background"
								options={{}}
							/>
						</Card>
					</div>
				)}
			</div>
		</Sheet>
	)
}

const getKey = (key: string) => `settingsScene.server/jobs.sections.history.inspector.${key}`
