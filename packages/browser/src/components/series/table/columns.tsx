import { Link, Text } from '@stump/components'
import { FileStatus, SeriesModelOrdering } from '@stump/graphql'
import { ColumnSort } from '@stump/sdk'
import { CellContext, ColumnDef, createColumnHelper } from '@tanstack/react-table'

import { usePaths } from '@/paths'

import CoverImageCell from './CoverImageCell'

export type SeriesTableData = {
	id: string
	resolvedName: string
	mediaCount: number
	percentageCompleted: number
	status: FileStatus
	oneshotBookId?: string
	/** The series' cover: its first stacked thumbnail */
	stackedThumbnails: {
		url: string
	}[]
}

const columnHelper = createColumnHelper<SeriesTableData>()

const coverColumn = columnHelper.display({
	cell: ({
		row: {
			original: { resolvedName, stackedThumbnails },
		},
	}) =>
		stackedThumbnails[0] ? (
			<CoverImageCell url={stackedThumbnails[0].url} title={resolvedName} />
		) : null,
	enableGlobalFilter: true,
	header: () => null,
	id: 'cover',
	size: 44,
})

function NameColumnCell({
	getValue,
	row: {
		original: { id },
	},
}: CellContext<SeriesTableData, string>) {
	const paths = usePaths()
	return (
		<Link
			to={paths.seriesOverview(id)}
			title={getValue()}
			className="text-sm min-w-0 truncate no-underline hover:opacity-90"
		>
			{getValue()}
		</Link>
	)
}

const nameColumn = columnHelper.accessor(({ resolvedName }) => resolvedName, {
	cell: (ctx) => <NameColumnCell {...ctx} />,
	enableGlobalFilter: true,
	enableSorting: true,
	header: () => (
		<Text size="sm" variant="secondary">
			Name
		</Text>
	),
	id: SeriesModelOrdering.Name,
	meta: { isGrow: true },
	size: 280,
})

const booksCountColumn = columnHelper.accessor((series) => series.mediaCount?.toString(), {
	cell: ({ getValue }) => (
		<Text size="sm" variant="muted">
			{getValue()}
		</Text>
	),
	enableGlobalFilter: true,
	// TODO(prisma 0.7.0): Support order by relation
	enableSorting: false,
	header: () => (
		<Text size="sm" variant="secondary">
			Books
		</Text>
	),
	id: 'media_count',
	size: 80,
})

// TODO: more columns

/**
 * A map of all columns that can be selected for the table. The key is the column ID, and the value is the column, itself.
 */
export const columnMap = {
	cover: coverColumn,
	name: nameColumn,
	books: booksCountColumn,
} as Record<string, ColumnDef<SeriesTableData>>

/** The columns offered in the column configuration, in the order they are listed */
export const columnOptionMap: Record<keyof typeof columnMap, string> = {
	cover: 'Cover',
	name: 'Name',
	books: 'Books',
}

/** The columns shown until the viewer picks their own (keys of `columnMap`) */
const defaultColumnKeys = ['cover', 'name', 'books']

export const defaultColumns = defaultColumnKeys.map(
	(key) => columnMap[key],
) as ColumnDef<SeriesTableData>[]

export const defaultColumnSort: ColumnSort[] = defaultColumnKeys.map((id, position) => ({
	id,
	position,
}))

/**
 * A helper function to build the columns for the table based on the stored column selection. If
 * no columns are selected, or if the selection is empty, the default columns will be used.
 */
export const buildColumns = (columns?: ColumnSort[]) => {
	if (!columns || columns.length === 0) {
		return defaultColumns
	}

	const sortedColumns = [...columns].sort((a, b) => a.position - b.position)
	const selectedColumnIds = sortedColumns.map(({ id }) => id)

	const selected = selectedColumnIds
		.map((id) => columnMap[id as keyof typeof columnMap])
		.filter(Boolean) as ColumnDef<SeriesTableData>[]
	return selected.length ? selected : defaultColumns
}
