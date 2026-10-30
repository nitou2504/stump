import { useSuspenseGraphQL } from '@stump/client'
import { Badge, Button, cn, Heading, Input, Text } from '@stump/components'
import { graphql, MetadataFacetField } from '@stump/graphql'
import { useMemo, useState } from 'react'
import { Helmet } from 'react-helmet'
import { Navigate, useParams, useSearchParams } from 'react-router'

import { SceneContainer } from '@/components/container'
import { Link } from '@/context'
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

	const hrefFor = (value: string) =>
		paths.bookSearchWithFilter({
			metadata:
				index === 'genres' ? { genres: { hasAnyOf: [value] } } : { writers: { hasAnyOf: [value] } },
		})

	return (
		<SceneContainer className="gap-4">
			<Helmet>
				<title>Stump | {title}</title>
			</Helmet>

			<div className="gap-3 md:flex-row md:items-end md:justify-between flex flex-col">
				<div>
					<Heading size="lg">{title}</Heading>
					<Text size="sm" variant="muted">
						{metadataValueCounts.length} {noun}
					</Text>
				</div>

				<div className="gap-2 flex items-center">
					<Input
						placeholder={`Filter ${noun}…`}
						value={search}
						onChange={(e) => setSearch(e.target.value)}
						className="md:w-64"
					/>
					<Button
						size="sm"
						variant={sort === 'name' ? 'secondary' : 'ghost'}
						onClick={() => setSort('name')}
					>
						A–Z
					</Button>
					<Button
						size="sm"
						variant={sort === 'count' ? 'secondary' : 'ghost'}
						onClick={() => setSort('count')}
					>
						Most books
					</Button>
				</div>
			</div>

			{sort === 'name' && sections.length > 1 && (
				<nav className="gap-1 top-0 py-2 sticky z-10 flex flex-wrap bg-background">
					{sections.map(({ title }) => (
						<a
							key={title}
							href={`#browse-${title}`}
							className="px-2 py-0.5 text-sm rounded-md text-muted-foreground hover:bg-accent hover:text-foreground"
							onClick={(e) => {
								e.preventDefault()
								document
									.getElementById(`browse-${title}`)
									?.scrollIntoView({ behavior: 'smooth', block: 'start' })
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

			{sections.map(({ title, entries }) => (
				<section key={title} id={`browse-${title}`} className="gap-2 scroll-mt-14 flex flex-col">
					<Heading size="sm" className={cn('pb-1 border-b border-border')}>
						{title}
						<span className="ml-2 text-sm font-normal text-muted-foreground">{entries.length}</span>
					</Heading>
					<div className="gap-1.5 flex flex-wrap">
						{entries.map(({ value, label, count }) => (
							<Link key={value} to={hrefFor(value)}>
								<Badge
									variant="secondary"
									rounded="full"
									className="gap-1.5 cursor-pointer hover:bg-accent"
								>
									<span>{label}</span>
									<span className="text-xs text-muted-foreground">{count}</span>
								</Badge>
							</Link>
						))}
					</div>
				</section>
			))}
		</SceneContainer>
	)
}
