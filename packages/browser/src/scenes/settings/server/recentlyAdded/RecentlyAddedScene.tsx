import { useGraphQL } from '@stump/client'
import { Button, Link, NativeSelect } from '@stump/components'
import { graphql } from '@stump/graphql'
import { formatDistanceToNow } from 'date-fns'
import { useEffect, useMemo, useState } from 'react'

import { SceneContainer } from '@/components/container'
import { SettingsSection } from '@/components/sharedLayout'
import { usePaths } from '@/paths'

const query = graphql(`
	query AdminRecentlyAddedBooks($userId: ID, $limit: Int! = 50) {
		recentlyAddedBooks(userId: $userId, limit: $limit) {
			itemId
			userId
			username
			title
			authors
			series
			paths
			libraryId
			libraryName
			filedAt
			mediaIds
		}
	}
`)

const PAGE = 50

/** Admin page: the books added through "Add books", who added them, when and where */
export default function RecentlyAddedScene() {
	const paths = usePaths()
	const [userId, setUserId] = useState<string>('')
	const [limit, setLimit] = useState(PAGE)
	const { data, isLoading } = useGraphQL(query, ['recentlyAddedBooks', userId, limit], {
		limit,
		userId: userId || undefined,
	})
	const rows = useMemo(() => data?.recentlyAddedBooks ?? [], [data])

	// remember every uploader seen so the filter keeps its options while filtered
	const [users, setUsers] = useState<Record<string, string>>({})
	useEffect(() => {
		const seen = Object.fromEntries(rows.map((row) => [row.userId, row.username ?? row.userId]))
		setUsers((prev) => ({ ...prev, ...seen }))
	}, [rows])

	return (
		<SceneContainer variant="settings">
			<SettingsSection
				title="Recently added"
				description='Books added through "Add books", newest first'
				actions={
					Object.keys(users).length > 1 || userId ? (
						<NativeSelect
							size="sm"
							className="w-44 text-base md:text-sm"
							value={userId}
							aria-label="Filter by user"
							options={[
								{ label: 'Everyone', value: '' },
								...Object.entries(users).map(([id, name]) => ({ label: name, value: id })),
							]}
							onChange={(e) => {
								setUserId(e.target.value)
								setLimit(PAGE)
							}}
						/>
					) : undefined
				}
			>
				{!isLoading && rows.length === 0 && (
					<p className="text-sm text-muted-foreground">
						Nothing has been added through “Add books” yet.
					</p>
				)}

				{rows.length > 0 && (
					<ul className="divide-y divide-border rounded-lg border border-border">
						{rows.map((row) => (
							<li
								key={row.itemId}
								className="gap-1 px-4 py-3 sm:gap-4 sm:flex-row sm:items-start flex flex-col"
							>
								<div className="min-w-0 flex flex-1 flex-col">
									<span className="font-medium">
										{row.mediaIds[0] ? (
											<Link to={paths.bookOverview(row.mediaIds[0])}>{row.title}</Link>
										) : (
											row.title
										)}
										{row.series && <span className="text-muted-foreground"> · {row.series}</span>}
									</span>
									{row.authors.length > 0 && (
										<span className="text-sm text-muted-foreground">{row.authors.join(', ')}</span>
									)}
									<span
										className="font-mono text-xs break-all text-muted-foreground"
										title={row.paths.join('\n')}
									>
										{row.libraryName} / {row.paths[0]}
										{row.paths.length > 1 && ` (+${row.paths.length - 1} more)`}
									</span>
								</div>
								<div className="gap-0.5 text-sm gap-x-2 sm:flex-col sm:items-end flex shrink-0 flex-row">
									<span>{row.username ?? 'Unknown user'}</span>
									<span
										className="text-muted-foreground"
										title={row.filedAt ? new Date(row.filedAt).toLocaleString() : undefined}
									>
										{row.filedAt
											? formatDistanceToNow(new Date(row.filedAt), { addSuffix: true })
											: ''}
									</span>
								</div>
							</li>
						))}
					</ul>
				)}

				{rows.length >= limit && (
					<div>
						<Button variant="outline" size="sm" onClick={() => setLimit((l) => l + PAGE)}>
							Show more
						</Button>
					</div>
				)}
			</SettingsSection>
		</SceneContainer>
	)
}
