import { useSuspenseGraphQL } from '@stump/client'
import { Badge, Button, Heading, Text } from '@stump/components'
import { graphql, MetadataFacetField } from '@stump/graphql'
import { useVirtualizer } from '@tanstack/react-virtual'
import { Tags, Users } from 'lucide-react'
import { useLayoutEffect, useMemo, useRef, useState } from 'react'
import { Helmet } from 'react-helmet'
import { Navigate, useParams, useSearchParams } from 'react-router'
import { useMediaMatch } from 'rooks'

import { SceneContainer } from '@/components/container'
import { FilterHeader, Search } from '@/components/filters'
import { CollectionSceneHeader } from '@/components/sharedLayout'
import { Link } from '@/context'
import { useScrollElement } from '@/hooks/useScrollElement'
import { BrowseIndex, usePaths } from '@/paths'
import { sortKey } from '@/utils/sortKey'
import { matchesSearch } from '@/utils/textSearch'

const query = graphql(`
	query BrowseIndexScene($field: MetadataFacetField!) {
		metadataValueCounts(field: $field) {
			value
			count
		}
	}
`)

const CONFIG: Record<BrowseIndex, { field: MetadataFacetField; title: string; noun: string }> = {
	authors: { field: MetadataFacetField.Writers, title: 'Authors', noun: 'authors' },
	genres: { field: MetadataFacetField.Genres, title: 'Genres', noun: 'genres' },
}

/** Values sharing a `Prefix: ` (e.g. `Shelf: Science`) are grouped once this many share it */
const MIN_PREFIX_GROUP = 3

type Entry = { value: string; label: string; count: number }
type Section = { title: string; entries: Entry[] }
type SortMode = 'name' | 'count'

/**
 * The index is virtualised: a library has thousands of authors, and rendering a link for each
 * (5,592 on the live server) meant ~36k page elements on a phone. The entries sit in a grid of
 * equal cells, so every row has a known height and only the rows on screen are rendered
 */
type Row =
	| { kind: 'heading'; key: string; section: Section }
	| { kind: 'entries'; key: string; entries: Entry[] }

const HEADING_HEIGHT = 48
const GAP = 6
/** Cells and rows are taller on touch screens, so every entry is a comfortable tap target */
const SIZES = {
	fine: { minCell: 180, rowHeight: 28 + GAP },
	coarse: { minCell: 150, rowHeight: 40 + GAP },
}

function buildRows(sections: Section[], columns: number): Row[] {
	const rows: Row[] = []
	for (const section of sections) {
		rows.push({ kind: 'heading', key: `h-${section.title}`, section })
		for (let start = 0; start < section.entries.length; start += columns) {
			rows.push({
				kind: 'entries',
				key: `e-${section.title}-${start}`,
				entries: section.entries.slice(start, start + columns),
			})
		}
	}
	return rows
}

const initialOf = (value: string) => {
	const first = sortKey(value).charAt(0)
	return /\p{L}/u.test(first) ? first.toUpperCase() : '#'
}

function buildSections(values: Entry[], sort: SortMode, groupPrefixes: boolean): Section[] {
	const sortEntries = (entries: Entry[]) =>
		[...entries].sort((a, b) =>
			sort === 'count'
				? b.count - a.count || sortKey(a.label).localeCompare(sortKey(b.label))
				: sortKey(a.label).localeCompare(sortKey(b.label)),
		)

	const prefixed = new Map<string, Entry[]>()
	if (groupPrefixes) {
		for (const entry of values) {
			const match = entry.value.match(/^([^:]{1,40}):\s+(.+)$/)
			if (match?.[1] && match[2]) {
				const list = prefixed.get(match[1]) ?? []
				list.push({ ...entry, label: match[2] })
				prefixed.set(match[1], list)
			}
		}
		for (const [prefix, list] of prefixed) {
			if (list.length < MIN_PREFIX_GROUP) prefixed.delete(prefix)
		}
	}

	const grouped = new Set([...prefixed.values()].flat().map(({ value }) => value))
	const prefixSections = [...prefixed.entries()]
		.sort(([a], [b]) => sortKey(a).localeCompare(sortKey(b)))
		.map(([title, entries]) => ({ title, entries: sortEntries(entries) }))

	const rest = values.filter(({ value }) => !grouped.has(value))
	if (sort === 'count') {
		return [...prefixSections, { title: 'All', entries: sortEntries(rest) }].filter(
			({ entries }) => entries.length,
		)
	}

	const letters = new Map<string, Entry[]>()
	for (const entry of rest) {
		const letter = initialOf(entry.value)
		letters.set(letter, [...(letters.get(letter) ?? []), entry])
	}
	const letterSections = [...letters.entries()]
		.sort(([a], [b]) => (a === '#' ? 1 : b === '#' ? -1 : a.localeCompare(b)))
		.map(([title, entries]) => ({ title, entries: sortEntries(entries) }))

	return [...prefixSections, ...letterSections]
}

export default function BrowseIndexScene() {
	const { index } = useParams<{ index: string }>()
	if (index !== 'genres' && index !== 'authors') {
		return <Navigate to="/404" replace />
	}
	return <BrowseIndexContent key={index} index={index} />
}

function BrowseIndexContent({ index }: { index: BrowseIndex }) {
	const paths = usePaths()
	const { field, title, noun } = CONFIG[index]

	const {
		data: { metadataValueCounts },
	} = useSuspenseGraphQL(query, ['browseIndex', field], { field })

	// The filter is kept in the URL (`?q=`), so going back to the index restores it
	const [searchParams, setSearchParams] = useSearchParams()
	const search = searchParams.get('q') ?? ''
	const setSearch = (value: string) =>
		setSearchParams(
			(prev) => {
				if (value) {
					prev.set('q', value)
				} else {
					prev.delete('q')
				}
				return prev
			},
			{ replace: true },
		)
	const [sort, setSort] = useState<SortMode>('name')

	const values = useMemo(() => {
		return metadataValueCounts
			.filter(({ value }) => matchesSearch(search, [value, sortKey(value)]))
			.map(({ value, count }) => ({ value, label: value, count }))
	}, [metadataValueCounts, search])

	const sections = useMemo(
		() => buildSections(values, sort, index === 'genres'),
		[values, sort, index],
	)

	const isCoarse = useMediaMatch('(pointer: coarse)')
	const { minCell, rowHeight } = isCoarse ? SIZES.coarse : SIZES.fine

	const listRef = useRef<HTMLDivElement>(null)
	const scrollElement = useScrollElement(listRef)
	const [listWidth, setListWidth] = useState(0)
	// Where the list starts inside the scrolling element (the page header, toolbar and letters
	// come first), which the virtualizer needs to place the rows
	const [scrollMargin, setScrollMargin] = useState(0)
	useLayoutEffect(() => {
		const list = listRef.current
		if (!list || !scrollElement) return
		const measure = () => {
			setListWidth(list.clientWidth)
			const offset =
				list.getBoundingClientRect().top -
				scrollElement.getBoundingClientRect().top +
				scrollElement.scrollTop
			setScrollMargin(Math.round(offset))
		}
		measure()
		const observer = new ResizeObserver(measure)
		observer.observe(list)
		return () => observer.disconnect()
	}, [scrollElement, sections.length])

	const columns = Math.max(1, Math.floor((listWidth + GAP) / (minCell + GAP)))
	const rows = useMemo(() => buildRows(sections, columns), [sections, columns])

	const virtualizer = useVirtualizer({
		count: rows.length,
		estimateSize: (index) => (rows[index]?.kind === 'heading' ? HEADING_HEIGHT : rowHeight),
		getItemKey: (index) => rows[index]?.key ?? index,
		getScrollElement: () => scrollElement,
		overscan: 6,
		scrollMargin,
	})

	// Scrolls a section's heading to just below the sticky page header, toolbar and letters,
	// whatever their height (the letters wrap on wide screens)
	const navRef = useRef<HTMLElement>(null)
	const jumpTo = (title: string) => {
		const index = rows.findIndex((row) => row.kind === 'heading' && row.section.title === title)
		const start = virtualizer.measurementsCache[index]?.start
		if (start == null || !scrollElement) return
		const nav = navRef.current
		const covered = nav ? parseFloat(getComputedStyle(nav).top) + nav.offsetHeight : 0
		scrollElement.scrollTo({ top: start - covered - 8, behavior: 'smooth' })
	}

	const hrefFor = (value: string) =>
		paths.bookSearchWithFilter({
			metadata:
				index === 'genres' ? { genres: { hasAnyOf: [value] } } : { writers: { hasAnyOf: [value] } },
		})

	return (
		<>
			<Helmet>
				<title>Stump | {title}</title>
			</Helmet>

			{/* Laid out like the list pages: the title, then the search with the sorting next to it */}
			<CollectionSceneHeader
				title={title}
				stats={[
					{ key: noun, icon: index === 'genres' ? Tags : Users, value: metadataValueCounts.length },
				]}
			/>

			<FilterHeader
				navOffset
				search={
					<Search
						initialValue={search}
						placeholder={`Search ${noun}…`}
						onChange={(value) => setSearch(value.trim())}
					/>
				}
				orderControls={
					<div className="gap-1 flex items-center">
						<Button
							size="sm"
							variant={sort === 'name' ? 'secondary' : 'ghost'}
							aria-pressed={sort === 'name'}
							onClick={() => setSort('name')}
						>
							A–Z
						</Button>
						<Button
							size="sm"
							variant={sort === 'count' ? 'secondary' : 'ghost'}
							aria-pressed={sort === 'count'}
							onClick={() => setSort('count')}
						>
							Most books
						</Button>
					</div>
				}
			/>

			<SceneContainer className="gap-4">
				{sort === 'name' && sections.length > 1 && (
					// One scrollable row on a phone: wrapped, the letters (plus the genre prefixes)
					// took several lines under the page header and toolbar, a third of the screen
					<nav
						ref={navRef}
						className="gap-1 py-2 top-24 md:top-12 md:flex-wrap md:overflow-visible sticky z-10 scrollbar-hide flex overflow-x-auto bg-background"
					>
						{sections.map(({ title }) => (
							<a
								key={title}
								href={`#browse-${title}`}
								className="px-2 py-0.5 text-sm shrink-0 rounded-md whitespace-nowrap text-muted-foreground hover:bg-accent hover:text-foreground"
								onClick={(e) => {
									e.preventDefault()
									jumpTo(title)
								}}
							>
								{title}
							</a>
						))}
					</nav>
				)}

				{!sections.length && (
					<Text variant="muted" className="py-8 text-center">
						Nothing matches “{search}”
					</Text>
				)}

				<div
					ref={listRef}
					className="relative w-full"
					style={{ height: virtualizer.getTotalSize() }}
				>
					{virtualizer.getVirtualItems().map((item) => {
						const row = rows[item.index]
						if (!row) return null
						return (
							<div
								key={item.key}
								className="left-0 absolute w-full"
								style={{ top: item.start - scrollMargin, height: item.size }}
							>
								{row.kind === 'heading' ? (
									<Heading
										id={`browse-${row.section.title}`}
										size="sm"
										className="pt-4 pb-1 border-b border-border"
									>
										{row.section.title}
										<span className="ml-2 text-sm font-normal text-muted-foreground">
											{row.section.entries.length}
										</span>
									</Heading>
								) : (
									<div
										className="pt-1.5 grid"
										style={{
											gap: GAP,
											gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))`,
										}}
									>
										{row.entries.map(({ value, label, count }) => (
											<Link key={value} to={hrefFor(value)} className="min-w-0 block" title={label}>
												<Badge
													variant="secondary"
													rounded="full"
													className="gap-1.5 h-7 pointer-coarse:h-10 px-2.5 w-full cursor-pointer justify-between hover:bg-accent"
												>
													<span className="truncate">{label}</span>
													<span className="text-xs text-muted-foreground">{count}</span>
												</Badge>
											</Link>
										))}
									</div>
								)}
							</div>
						)
					})}
				</div>
			</SceneContainer>
		</>
	)
}
