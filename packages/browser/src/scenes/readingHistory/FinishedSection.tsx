import { cn, Text } from '@stump/components'
import { extractErrorMessage } from '@stump/graphql'
import { useCallback, useMemo } from 'react'
import { useSearchParams } from 'react-router'

import { Search } from '@/components/filters'
import GenericEmptyState from '@/components/GenericEmptyState'
import Pagination from '@/components/Pagination'
import { ThumbnailImage } from '@/components/thumbnail'
import { Link } from '@/context'
import { useTranslate } from '@/hooks/useTranslate'
import { usePaths } from '@/paths'

import { formatDay, formatMonth, monthOf } from './dates'
import { useFinished } from './graphql'

type Props = {
	/** A friend's id, or nothing for the person's own page */
	userId?: string
}

/** `?year=2025` as a number, or nothing when it is missing or isn't a year */
const parseYear = (value: string | null) => {
	const year = Number(value)
	return value && Number.isInteger(year) && year > 0 ? year : undefined
}

/** `?page=3` as a page number: the first when it is missing or isn't one */
const parsePage = (value: string | null) => {
	const page = Number(value)
	return Number.isInteger(page) && page > 0 ? page : 1
}

/**
 * The books the reader has finished, the last one finished first, in months. It can be searched
 * and narrowed to a year, and read a page at a time; all of that is in the address (`?search=`,
 * `?year=`, `?page=`), so going back to the page restores it. The years and their counts, and
 * the books, are what the person looking may read of the reader's history
 */
export default function FinishedSection({ userId }: Props) {
	const { t } = useTranslate()
	const paths = usePaths()
	const [params, setParams] = useSearchParams()

	const search = params.get('search') ?? ''
	const year = parseYear(params.get('year'))
	const page = parsePage(params.get('page'))

	const { data, isLoading, error } = useFinished(userId, { search, year, page })

	/** Changes some of the page's address, and starts over from the first page */
	const change = useCallback(
		(values: { search?: string; year?: number | null }, options?: { replace?: boolean }) =>
			setParams(
				(previous) => {
					const next = new URLSearchParams(previous)
					for (const [key, value] of Object.entries(values)) {
						if (value) next.set(key, String(value))
						else next.delete(key)
					}
					next.delete('page')
					return next
				},
				{ replace: options?.replace },
			),
		[setParams],
	)

	const setPage = useCallback(
		(next: number) =>
			setParams((previous) => {
				const params = new URLSearchParams(previous)
				if (next > 1) params.set('page', String(next))
				else params.delete('page')
				return params
			}),
		[setParams],
	)

	const history = data?.readingHistory
	const years = history?.years ?? []
	const nodes = history?.finished.nodes
	const pageInfo = history?.finished.pageInfo
	const total = years.reduce((count, { count: forYear }) => count + forYear, 0)

	// the books of a page in months: they come in order, so a month is a run of them
	const months = useMemo(() => {
		const groups: { month: string; nodes: NonNullable<typeof nodes> }[] = []
		for (const node of nodes ?? []) {
			const month = monthOf(node.finishedOn)
			const last = groups[groups.length - 1]
			if (last?.month === month) last.nodes.push(node)
			else groups.push({ month, nodes: [node] })
		}
		return groups
	}, [nodes])

	const hasFilters = !!search || year != null

	const renderBooks = () => {
		if (error && !history) {
			return (
				<GenericEmptyState
					title={t('readingHistory.errors.load')}
					subtitle={extractErrorMessage(error)}
				/>
			)
		}
		if (isLoading || !history) return null

		if (!months.length) {
			return (
				<GenericEmptyState
					title={
						hasFilters ? t('readingHistory.finished.noMatch') : t('readingHistory.finished.empty')
					}
					subtitle={
						hasFilters
							? t('readingHistory.finished.noMatchHint')
							: t('readingHistory.finished.emptyHint')
					}
				/>
			)
		}

		return (
			<div className="gap-6 flex flex-col">
				{months.map(({ month, nodes: monthNodes }) => (
					<section key={month} aria-label={formatMonth(month)} className="gap-3 flex flex-col">
						<h3 className="pb-1 text-base font-medium border-b border-border">
							{formatMonth(month)}
							<span className="ml-2 text-sm font-normal text-muted-foreground">
								{monthNodes.length}
							</span>
						</h3>
						<ul className="gap-3 sm:grid-cols-2 xl:grid-cols-3 grid grid-cols-1">
							{monthNodes.map(({ media, finishedOn, readthroughNumber }) => (
								<li key={`${media.id}-${readthroughNumber}`}>
									<Link
										to={paths.bookOverview(media.id)}
										className="gap-3 p-2 flex items-center rounded-lg hover:bg-accent"
									>
										<ThumbnailImage
											src={media.thumbnail.url}
											alt=""
											size={{ width: 48, height: 72 }}
											className="shrink-0"
											lazy
										/>
										<span className="gap-0.5 min-w-0 flex flex-col">
											<Text className="text-sm font-medium leading-tight line-clamp-2">
												{media.resolvedName}
											</Text>
											{!!media.metadata?.writers.length && (
												<Text variant="muted" size="xs" className="truncate">
													{media.metadata.writers.join(', ')}
												</Text>
											)}
											<span className="gap-2 flex flex-wrap items-center">
												<Text variant="muted" size="xs">
													{formatDay(finishedOn)}
												</Text>
											</span>
										</span>
									</Link>
								</li>
							))}
						</ul>
					</section>
				))}
			</div>
		)
	}

	return (
		<section aria-label={t('readingHistory.finished.heading')} className="gap-4 flex flex-col">
			<h2 className="text-lg font-semibold">
				{t('readingHistory.finished.heading')}
				{!!history && (
					<span className="ml-2 text-sm font-normal text-muted-foreground">
						{t('readingHistory.finished.count', { count: total })}
					</span>
				)}
			</h2>

			<div className="gap-3 sm:flex-row sm:items-center flex flex-col">
				<Search
					initialValue={search}
					placeholder={t('readingHistory.finished.search')}
					onChange={(value) => {
						// the box reports its value once when it mounts: that is the address's own search, and
						// acting on it would start over from the first page, so only a different search counts
						const next = value.trim()
						if (next !== search.trim()) change({ search: next }, { replace: true })
					}}
					isLoading={isLoading}
					className="sm:max-w-xs"
				/>

				{years.length > 0 && (
					<nav
						aria-label={t('readingHistory.finished.years')}
						className="gap-1 flex flex-wrap items-center"
					>
						<YearChip isActive={year == null} onClick={() => change({ year: null })}>
							{t('readingHistory.finished.allYears')}
						</YearChip>
						{years.map(({ year: forYear, count }) => (
							<YearChip
								key={forYear}
								isActive={year === forYear}
								onClick={() => change({ year: forYear })}
							>
								{forYear}
								<span className="ml-1.5 text-xs text-muted-foreground">{count}</span>
							</YearChip>
						))}
					</nav>
				)}
			</div>

			{renderBooks()}

			{!!pageInfo && pageInfo.totalPages > 1 && (
				<Pagination
					position="bottom"
					pages={pageInfo.totalPages}
					currentPage={pageInfo.currentPage}
					onChangePage={setPage}
				/>
			)}
		</section>
	)
}

function YearChip({
	isActive,
	onClick,
	children,
}: {
	isActive: boolean
	onClick: () => void
	children: React.ReactNode
}) {
	return (
		<button
			type="button"
			aria-pressed={isActive}
			onClick={onClick}
			className={cn(
				'px-3 py-1 text-sm rounded-full border border-border transition-colors hover:bg-accent',
				isActive && 'font-medium border-transparent bg-secondary text-foreground',
			)}
		>
			{children}
		</button>
	)
}
