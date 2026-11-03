import { useGraphQL } from '@stump/client'
import { Badge, Button, cn, Text } from '@stump/components'
import { graphql, PostProcessKind, PostProcessStatus } from '@stump/graphql'
import { formatDistanceToNow } from 'date-fns'
import { RefreshCw } from 'lucide-react'

const query = graphql(`
	query PostProcessingTasks($libraryId: ID, $limit: Int) {
		postProcessingTasks(libraryId: $libraryId, limit: $limit) {
			id
			libraryId
			mediaId
			kind
			steps
			status
			path
			reason
			bytesBefore
			bytesAfter
			updatedAt
		}
	}
`)

const STATUS_LABEL: Record<PostProcessStatus, string> = {
	[PostProcessStatus.Pending]: 'Queued',
	[PostProcessStatus.Running]: 'Running',
	[PostProcessStatus.Done]: 'Replaced',
	[PostProcessStatus.Rejected]: 'Kept',
	[PostProcessStatus.Skipped]: 'Nothing to do',
	[PostProcessStatus.Failed]: 'Failed',
	[PostProcessStatus.Estimated]: 'Estimate',
	[PostProcessStatus.Reverted]: 'Original put back',
}

const STATUS_VARIANT: Record<PostProcessStatus, 'secondary' | 'primary' | 'success' | 'error'> = {
	[PostProcessStatus.Pending]: 'secondary',
	[PostProcessStatus.Running]: 'primary',
	[PostProcessStatus.Done]: 'success',
	[PostProcessStatus.Rejected]: 'secondary',
	[PostProcessStatus.Skipped]: 'secondary',
	[PostProcessStatus.Failed]: 'error',
	[PostProcessStatus.Estimated]: 'primary',
	[PostProcessStatus.Reverted]: 'secondary',
}

export const formatBytes = (bytes?: number | null) =>
	bytes == null ? '' : `${(bytes / 1e6).toFixed(bytes < 10e6 ? 1 : 0)} MB`

const fileName = (path: string) => path.split('/').pop() || path

const kindLabel = (kind: PostProcessKind, steps?: string | null) =>
	kind === PostProcessKind.Grey_16
		? '16 greys'
		: `PDF ${(steps || '').toLowerCase().replace('+', ' + ')}`.trim()

type Props = {
	/** Only this library's tasks (every library the user can see otherwise) */
	libraryId?: string
	limit?: number
	className?: string
}

/**
 * Post-processing tasks (16 greys, PDF OCR/shrink), most recent first. Refreshes every few
 * seconds while something is queued or running
 */
export default function PostProcessingTasks({ libraryId, limit = 50, className }: Props) {
	const { data, isLoading, refetch, isFetching } = useGraphQL(
		query,
		['postProcessingTasks', libraryId ?? 'all', limit],
		{ libraryId, limit },
		{
			refetchInterval: (q) =>
				q.state.data?.postProcessingTasks.some(
					(t) => t.status === PostProcessStatus.Pending || t.status === PostProcessStatus.Running,
				)
					? 5000
					: false,
		},
	)

	const tasks = data?.postProcessingTasks ?? []

	return (
		<div className={cn('gap-2 flex flex-col', className)}>
			<div className="flex items-center justify-between">
				<Text size="sm" variant="muted">
					{tasks.length ? `Last ${tasks.length} tasks` : ''}
				</Text>
				<Button size="sm" variant="ghost" onClick={() => refetch()} disabled={isFetching}>
					<RefreshCw className={cn('mr-1.5 h-3.5 w-3.5', isFetching && 'animate-spin')} />
					Refresh
				</Button>
			</div>

			{!isLoading && tasks.length === 0 && (
				<Text size="sm" variant="muted">
					Nothing processed yet.
				</Text>
			)}

			{tasks.length > 0 && (
				<ul className="divide-y divide-border rounded-lg border border-border">
					{tasks.map((task) => (
						<li
							key={task.id}
							className="gap-x-3 gap-y-1 px-3 py-2 sm:flex-row sm:items-center flex flex-col"
						>
							<div className="min-w-0 flex-1">
								<Text size="sm" className="font-medium line-clamp-1" title={task.path}>
									{fileName(task.path)}
								</Text>
								<Text size="xs" variant="muted" className="line-clamp-2" title={task.reason ?? ''}>
									{kindLabel(task.kind, task.steps)}
									{task.reason ? ` · ${task.reason}` : ''}
								</Text>
							</div>
							<div className="gap-2 flex shrink-0 items-center">
								{task.bytesBefore != null && (
									<Text size="xs" variant="muted" className="whitespace-nowrap tabular-nums">
										{formatBytes(task.bytesBefore)}
										{task.bytesAfter != null && ` → ${formatBytes(task.bytesAfter)}`}
									</Text>
								)}
								<Text
									size="xs"
									variant="muted"
									className="whitespace-nowrap"
									title={new Date(task.updatedAt).toLocaleString()}
								>
									{formatDistanceToNow(new Date(task.updatedAt), { addSuffix: true })}
								</Text>
								<Badge size="xs" variant={STATUS_VARIANT[task.status]}>
									{STATUS_LABEL[task.status]}
								</Badge>
							</div>
						</li>
					))}
				</ul>
			)}
		</div>
	)
}
