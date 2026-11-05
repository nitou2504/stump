import { Link, Text } from '@stump/components'
import { Media } from '@stump/graphql'
import { ColumnSort } from '@stump/sdk'
import { ColumnDef, createColumnHelper } from '@tanstack/react-table'
import { CellContext } from '@tanstack/react-table'
import { format, intlFormat, isValid } from 'date-fns'

import { LanguageCell } from '@/components/book/table/columns'
import { usePaths } from '@/paths'
import { bookLanguageCode } from '@/utils/language'
import { matchesSearch } from '@/utils/textSearch'

import BookLinksCell from './BookLinksCell'
import CoverImageCell from './CoverImageCell'

const columnHelper = createColumnHelper<Media>()

/** A single-line cell: long values end with an ellipsis and show in full on hover */
function TextCell({ value }: { value?: string | number | null }) {
	if (value == null || value === '') return null
	return (
		<Text size="sm" variant="muted" className="truncate" title={String(value)}>
			{value}
		</Text>
	)
}

const header = (label: string) =>
	function Header() {
		return (
			<Text size="sm" variant="muted" className="truncate">
				{label}
			</Text>
		)
	}

const coverColumn = columnHelper.display({
	cell: ({ row: { original: book } }) => (
		<CoverImageCell url={book.thumbnail.url} title={book.resolvedName} />
	),
	enableGlobalFilter: true,
	header: () => null,
	id: 'cover',
	size: 52,
})

function NameColumnCell({
	getValue,
	row: {
		original: { id },
	},
}: CellContext<Media, string>) {
	const paths = usePaths()
	return (
		<Link
			to={paths.bookOverview(id)}
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
	header: header('Name'),
	id: 'name',
	meta: { isGrow: true },
	size: 240,
})

const pagesColumn = columnHelper.accessor('pages', {
	cell: ({ getValue }) => <TextCell value={getValue()} />,
	enableGlobalFilter: true,
	enableSorting: true,
	header: header('Pages'),
	id: 'pages',
	size: 92,
})

const publishedColumn = columnHelper.accessor(
	({ metadata }) => {
		const { year, month, day } = metadata || {}

		// TODO: validation
		if (!!year && !!month && !!day) {
			return format(new Date(year, month - 1, day), 'yyyy-MM-dd')
		} else if (!!year && !!month) {
			return format(new Date(year, month - 1), 'yyyy-MM')
		} else if (year) {
			return String(year)
		}

		return ''
	},
	{
		cell: ({ getValue }) => <TextCell value={getValue()} />,
		enableGlobalFilter: true,
		enableSorting: true,
		header: header('Published'),
		id: 'published',
		size: 124,
	},
)

const addedColumn = columnHelper.accessor(
	({ createdAt }) => {
		const date = new Date(createdAt)
		if (!isValid(date)) return ''
		return intlFormat(date, {
			year: 'numeric',
			month: 'numeric',
			day: 'numeric',
		})
	},
	{
		cell: ({ getValue }) => <TextCell value={getValue()} />,
		enableGlobalFilter: true,
		enableSorting: true,
		header: header('Added'),
		id: 'added',
		size: 116,
	},
)

const publisherColumn = columnHelper.accessor(({ metadata }) => metadata?.publisher, {
	cell: ({ getValue }) => <TextCell value={getValue()} />,
	enableGlobalFilter: true,
	enableSorting: true,
	header: header('Publisher'),
	id: 'publisher',
	meta: { isGrow: true },
	size: 140,
})

const ageRatingColumn = columnHelper.accessor(({ metadata }) => metadata?.ageRating, {
	cell: ({ getValue }) => <TextCell value={getValue()} />,
	enableGlobalFilter: true,
	enableSorting: true,
	header: header('Age Rating'),
	id: 'age_rating',
	size: 112,
})

const languageColumn = columnHelper.accessor(
	({ metadata }) => bookLanguageCode(metadata?.language) ?? undefined,
	{
		cell: ({ getValue }) => <LanguageCell value={getValue()} />,
		enableGlobalFilter: true,
		enableSorting: true,
		header: header('Language'),
		id: 'language',
		size: 96,
		sortUndefined: 'last',
	},
)

const ratingColumn = columnHelper.accessor(
	({ metadata }) => (metadata?.rating != null ? Number(metadata.rating) : undefined),
	{
		cell: ({ getValue }) => {
			const value = getValue()
			if (value == null || isNaN(value) || value <= 0) return null
			return <TextCell value={`★ ${value.toFixed(1)}`} />
		},
		enableGlobalFilter: false,
		enableSorting: true,
		header: header('Rating'),
		id: 'rating',
		size: 96,
		// unrated books sort last in both directions
		sortUndefined: 'last',
	},
)

const positionColumn = columnHelper.accessor(
	({ metadata }) => (metadata?.number != null ? Number(metadata.number) : undefined),
	{
		cell: ({ getValue }) => {
			const value = getValue()
			if (value == null || isNaN(value)) return null
			return <TextCell value={`#${Number(value.toFixed(2))}`} />
		},
		enableGlobalFilter: false,
		enableSorting: true,
		header: header('#'),
		id: 'position',
		size: 64,
		sortUndefined: 'last',
	},
)

const seriesColumn = columnHelper.accessor(
	({ metadata }) => {
		if (!metadata?.series) return undefined
		const number = metadata.number != null ? Number(metadata.number) : NaN
		return isNaN(number) ? metadata.series : `${metadata.series} #${number}`
	},
	{
		cell: ({ getValue }) => <TextCell value={getValue()} />,
		enableGlobalFilter: true,
		enableSorting: true,
		header: header('Series'),
		id: 'series',
		meta: { isGrow: true },
		size: 160,
		sortUndefined: 'last',
	},
)

/** A column of values joined on one line: people, genres, ... */
const listColumn = (
	id: string,
	label: string,
	pick: (book: Media) => string[] | null | undefined,
	size = 140,
) =>
	columnHelper.accessor((book) => pick(book)?.join(', '), {
		cell: ({ getValue }) => <TextCell value={getValue()} />,
		enableGlobalFilter: true,
		enableSorting: true,
		header: header(label),
		id,
		meta: { isGrow: true },
		size,
	})

const genresColumn = listColumn('genres', 'Genres', ({ metadata }) => metadata?.genres, 200)
const writersColumn = listColumn('writers', 'Writers', ({ metadata }) => metadata?.writers, 160)
const inkersColumn = listColumn('inkers', 'Inkers', ({ metadata }) => metadata?.inkers)
const pencillersColumn = listColumn(
	'pencillers',
	'Pencillers',
	({ metadata }) => metadata?.pencillers,
)
const coloristsColumn = listColumn('colorists', 'Colorists', ({ metadata }) => metadata?.colorists)
const letterersColumn = listColumn('letterers', 'Letterers', ({ metadata }) => metadata?.letterers)
const artistsColumn = listColumn('artists', 'Artists', ({ metadata }) => metadata?.coverArtists)
const charactersColumn = listColumn(
	'characters',
	'Characters',
	({ metadata }) => metadata?.characters,
)

const volumeColumn = columnHelper.accessor(({ metadata }) => metadata?.volume, {
	cell: ({ getValue }) => <TextCell value={getValue()} />,
	enableGlobalFilter: true,
	enableSorting: true,
	header: header('Volume'),
	id: 'volume',
	size: 92,
})

const linksColumn = columnHelper.accessor(({ metadata }) => metadata?.links?.join(', '), {
	cell: ({
		row: {
			original: { metadata },
		},
	}) => <BookLinksCell links={metadata?.links || []} />,

	enableGlobalFilter: true,
	enableSorting: true,
	header: header('Links'),
	id: 'links',
	meta: { isGrow: true },
	size: 160,
})

export type MediaTableColumnDef = ColumnDef<Media>

/**
 * A map of all columns that can be selected for the table. The key is the column ID, and the value is the column, itself.
 */
export const columnMap = {
	cover: coverColumn,
	name: nameColumn,
	position: positionColumn,
	writers: writersColumn,
	series: seriesColumn,
	language: languageColumn,
	genres: genresColumn,
	rating: ratingColumn,
	age_rating: ageRatingColumn,
	pages: pagesColumn,
	published: publishedColumn,
	added: addedColumn,
	publisher: publisherColumn,
	volume: volumeColumn,
	artists: artistsColumn,
	characters: charactersColumn,
	colorists: coloristsColumn,
	inkers: inkersColumn,
	letterers: letterersColumn,
	pencillers: pencillersColumn,
	links: linksColumn,
} as Record<string, ColumnDef<Media>>

// TODO: localization keys instead of hardcoded strings
/** The columns offered in the column configuration, in the order they are listed */
export const columnOptionMap: Record<keyof typeof columnMap, string> = {
	cover: 'Cover',
	name: 'Name',
	position: 'Series position',
	writers: 'Writers',
	series: 'Series',
	language: 'Language',
	genres: 'Genres',
	rating: 'Rating',
	age_rating: 'Age Rating',
	pages: 'Pages',
	published: 'Published',
	added: 'Added',
	publisher: 'Publisher',
	volume: 'Volume',
	artists: 'Artists',
	characters: 'Characters',
	colorists: 'Colorists',
	inkers: 'Inkers',
	letterers: 'Letterers',
	pencillers: 'Pencillers',
	links: 'Links',
}

export const defaultColumns = [
	coverColumn,
	nameColumn,
	writersColumn,
	languageColumn,
	pagesColumn,
	publishedColumn,
	addedColumn,
] as ColumnDef<Media>[]

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
		.filter(Boolean) as ColumnDef<Media>[]
	return selected.length ? selected : defaultColumns
}

/**
 * Whether a book matches a smart list search: every word starts a word in its name, file
 * name, title, people, series, publisher, genres, tags or a few numbers, ignoring case and
 * accents
 */
export const bookFuzzySearch = (book: Media, search: string): boolean => {
	const { resolvedName, metadata } = book
	return matchesSearch(search, [
		resolvedName,
		book.path?.split('/').pop(),
		metadata?.title,
		metadata?.series,
		metadata?.publisher,
		metadata?.ageRating,
		metadata?.volume,
		metadata?.number,
		...(metadata?.writers ?? []),
		...(metadata?.pencillers ?? []),
		...(metadata?.inkers ?? []),
		...(metadata?.colorists ?? []),
		...(metadata?.letterers ?? []),
		...(metadata?.coverArtists ?? []),
		...(metadata?.editors ?? []),
		...(metadata?.genres ?? []),
		...(metadata?.links ?? []),
		...(book.tags ?? []).map((tag) => tag.name),
	])
}
