import { formatBytes } from '@stump/client'
import { Link, Text } from '@stump/components'
import { FragmentType, Media, MediaMetadataModelOrdering, MediaModelOrdering } from '@stump/graphql'
import { useLocaleContext } from '@stump/i18n'
import { ColumnSort } from '@stump/sdk'
import { CellContext, ColumnDef, createColumnHelper } from '@tanstack/react-table'
import { format, intlFormat, isValid } from 'date-fns'

import { usePaths } from '@/paths'
import { bookLanguage } from '@/utils/language'
import { isEbookExtension } from '@/utils/readingProgress'

import { BookCardFragment } from '../BookCard'
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

/** A list of values (people, genres) on a single line */
function ListCell({ values }: { values?: string[] | null }) {
	if (!values?.length) return null
	return <TextCell value={values.join(', ')} />
}

/** A book's language as its short code (`ES`), named in full on hover */
export function LanguageCell({ value }: { value?: string | null }) {
	const { locale } = useLocaleContext()
	const language = bookLanguage(value, locale)
	if (!language) return null
	return (
		<Text size="sm" variant="muted" className="truncate" title={language.name}>
			{language.label}
		</Text>
	)
}

const header = (label: string) =>
	function Header() {
		return (
			<Text size="sm" variant="secondary" className="truncate">
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
	size: 44,
})

function NameColumnCell({
	getValue,
	row: {
		original: { id, extension, libraryConfig, readProgress },
	},
}: CellContext<Media, string>) {
	const paths = usePaths()
	return (
		<Link
			to={
				libraryConfig?.skipBookOverview
					? paths.bookReader(id, {
							isEpub: isEbookExtension(extension),
							page: isEbookExtension(extension) ? undefined : (readProgress?.page ?? undefined),
						})
					: paths.bookOverview(id)
			}
			title={getValue()}
			className="text-sm min-w-0 truncate no-underline hover:text-foreground/90"
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
	id: MediaModelOrdering.Name, // TODO (graphql): should this be resovledName?, sorting by `name` is different from sorting by `resolvedName`
	meta: { isGrow: true },
	size: 240,
})

const pagesColumn = columnHelper.accessor('pages', {
	cell: ({ getValue }) => <TextCell value={getValue()} />,
	enableGlobalFilter: true,
	enableSorting: true,
	header: header('Pages'),
	id: MediaModelOrdering.Pages,
	size: 84,
})

const fileSizeColumn = columnHelper.accessor('size', {
	cell: ({ getValue }) => <TextCell value={formatBytes(getValue())} />,
	enableGlobalFilter: true,
	enableSorting: true,
	header: header('File Size'),
	id: MediaModelOrdering.Size,
	size: 100,
})

const extensionColumn = columnHelper.accessor('extension', {
	cell: ({ getValue }) => <TextCell value={getValue()} />,
	enableGlobalFilter: true,
	enableSorting: true,
	header: header('Extension'),
	id: MediaModelOrdering.Extension,
	size: 108,
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
		// TODO(sorting): Consider better null/empty handling (e.g., placing nulls last or excluding them when sorting)
		enableSorting: true,
		header: header('Published'),
		id: MediaMetadataModelOrdering.Year,
		size: 116,
	},
)

const addedColumn = columnHelper.accessor(
	({ createdAt }) => {
		const date = new Date(createdAt)
		return isValid(date)
			? intlFormat(date, { year: 'numeric', month: 'numeric', day: 'numeric' })
			: ''
	},
	{
		cell: ({
			getValue,
			row: {
				original: { createdAt },
			},
		}) => {
			const date = new Date(createdAt)
			return (
				<Text
					size="sm"
					variant="muted"
					className="truncate"
					title={isValid(date) ? intlFormat(date, { dateStyle: 'long', timeStyle: 'short' }) : ''}
				>
					{getValue()}
				</Text>
			)
		},
		enableGlobalFilter: true,
		enableSorting: true,
		header: header('Added'),
		id: MediaModelOrdering.CreatedAt,
		size: 108,
	},
)

const publisherColumn = columnHelper.accessor(({ metadata }) => metadata?.publisher, {
	cell: ({ getValue }) => <TextCell value={getValue()} />,
	enableGlobalFilter: true,
	// TODO(relation-ordering): Support order by relation
	enableSorting: false,
	header: header('Publisher'),
	id: 'publisher',
	meta: { isGrow: true },
	size: 140,
})

const ageRatingColumn = columnHelper.accessor(({ metadata }) => metadata?.ageRating, {
	cell: ({ getValue }) => <TextCell value={getValue()} />,
	enableGlobalFilter: true,
	// TODO(relation-ordering): Support order by relation
	enableSorting: false,
	header: header('Age Rating'),
	id: 'age_rating',
	size: 96,
})

const languageColumn = columnHelper.accessor(({ metadata }) => metadata?.language, {
	cell: ({ getValue }) => <LanguageCell value={getValue()} />,
	enableGlobalFilter: true,
	enableSorting: true,
	header: header('Language'),
	id: MediaMetadataModelOrdering.Language,
	size: 92,
})

const genresColumn = columnHelper.accessor(({ metadata }) => metadata?.genres, {
	cell: ({ getValue }) => <ListCell values={getValue()} />,
	enableGlobalFilter: true,
	// TODO(relation-ordering): Support order by relation
	enableSorting: false,
	header: header('Genres'),
	id: 'genres',
	meta: { isGrow: true },
	size: 200,
})

const seriesColumn = columnHelper.accessor(
	({ metadata }) => {
		if (!metadata?.series) return undefined
		const number = metadata.number != null ? Number(metadata.number) : NaN
		return isNaN(number) ? metadata.series : `${metadata.series} #${number}`
	},
	{
		cell: ({ getValue }) => <TextCell value={getValue()} />,
		enableGlobalFilter: true,
		// TODO(relation-ordering): Support order by relation
		enableSorting: false,
		header: header('Series'),
		id: 'series',
		meta: { isGrow: true },
		size: 160,
	},
)

const volumeColumn = columnHelper.accessor(({ metadata }) => metadata?.volume, {
	cell: ({ getValue }) => <TextCell value={getValue()} />,
	enableGlobalFilter: true,
	// TODO(relation-ordering): Support order by relation
	enableSorting: false,
	header: header('Volume'),
	id: 'volume',
	size: 80,
})

const peopleColumn = (id: string, label: string, pick: (book: Media) => string[] | undefined) =>
	columnHelper.accessor((book) => pick(book), {
		cell: ({ getValue }) => <ListCell values={getValue()} />,
		enableGlobalFilter: true,
		// TODO(relation-ordering): Support order by relation
		enableSorting: false,
		header: header(label),
		id,
		meta: { isGrow: true },
		size: id === 'writers' ? 160 : 140,
	})

const writersColumn = peopleColumn('writers', 'Writers', ({ metadata }) => metadata?.writers)
const inkersColumn = peopleColumn('inkers', 'Inkers', ({ metadata }) => metadata?.inkers)
const pencillersColumn = peopleColumn(
	'pencillers',
	'Pencillers',
	({ metadata }) => metadata?.pencillers,
)
const coloristsColumn = peopleColumn(
	'colorists',
	'Colorists',
	({ metadata }) => metadata?.colorists,
)
const letterersColumn = peopleColumn(
	'letterers',
	'Letterers',
	({ metadata }) => metadata?.letterers,
)
const artistsColumn = peopleColumn('artists', 'Artists', ({ metadata }) => metadata?.coverArtists)
const charactersColumn = peopleColumn(
	'characters',
	'Characters',
	({ metadata }) => metadata?.characters,
)

const linksColumn = columnHelper.accessor(({ metadata }) => metadata?.links?.join(', '), {
	cell: ({
		row: {
			original: { metadata },
		},
	}) => <BookLinksCell links={metadata?.links || []} />,

	enableGlobalFilter: true,
	// TODO(relation-ordering): Support order by relation
	enableSorting: false,
	header: header('Links'),
	id: 'links',
	meta: { isGrow: true },
	size: 160,
})

const ratingColumn = columnHelper.accessor(
	({ metadata }) => (metadata?.rating != null ? Number(metadata.rating) : null),
	{
		cell: ({ getValue }) => {
			const value = getValue()
			if (value == null || isNaN(value) || value <= 0) return null
			return <TextCell value={`★ ${value.toFixed(1)}`} />
		},
		enableGlobalFilter: false,
		enableSorting: true,
		header: header('Rating'),
		id: MediaMetadataModelOrdering.Rating,
		size: 92,
	},
)

const positionColumn = columnHelper.accessor(
	({ metadata, seriesPosition }) => seriesPosition ?? Number(metadata?.number),
	{
		cell: ({ getValue }) => {
			const value = getValue()
			if (value == null || isNaN(value)) return null

			return (
				<Text size="sm" variant="muted" className="w-full truncate text-right">
					{value}
				</Text>
			)
		},
		enableGlobalFilter: true,
		// TODO(relation-ordering): Support order by relation
		enableSorting: false,
		header: () => (
			<Text size="sm" variant="muted" className="w-full text-center">
				#
			</Text>
		),
		id: 'position',
		size: 48,
	},
)

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
	extension: extensionColumn,
	file_size: fileSizeColumn,
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
	extension: 'Extension',
	file_size: 'File Size',
}

/** The columns shown until the viewer picks their own (keys of `columnMap`) */
const defaultColumnKeys = ['cover', 'name', 'writers', 'language', 'pages', 'published', 'added']

export const defaultColumns = defaultColumnKeys.map((key) => columnMap[key]) as ColumnDef<
	FragmentType<typeof BookCardFragment>
>[]

/**
 * The default selection, by `columnMap` key (the column configuration and `buildColumns`
 * work with keys, the column IDs are the server's ordering fields)
 */
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
		.filter(Boolean) as ColumnDef<FragmentType<typeof BookCardFragment>>[]
	return selected.length ? selected : defaultColumns
}
