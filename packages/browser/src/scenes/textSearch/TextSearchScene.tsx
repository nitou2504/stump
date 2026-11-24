import { Button, Text } from '@stump/components'
import { Helmet } from 'react-helmet'
import { Link, useSearchParams } from 'react-router'

import { highlight } from '@/components/search/highlight'
import { useContentSearch } from '@/components/search/useContentSearch'
import { PageHeader } from '@/components/sharedLayout'
import { useTranslate } from '@/hooks/useTranslate'
import { usePaths } from '@/paths'

const PAGE_SIZE = 20

/**
 * Search inside the books: the passages of the text that match, the best of each book, with the
 * book they are in. A hit opens the book.
 */
export default function TextSearchScene() {
	const { t } = useTranslate()
	const paths = usePaths()
	const [params, setParams] = useSearchParams()
	const q = (params.get('q') ?? '').trim()
	const page = Math.max(1, Number(params.get('page')) || 1)
	const { data, isFetching, isError } = useContentSearch(q, true, PAGE_SIZE, page)
	const totalPages = data ? Math.max(1, Math.ceil(data.total / PAGE_SIZE)) : 1

	const go = (next: number) =>
		setParams((prev) => {
			prev.set('page', String(next))
			return prev
		})

	return (
		<div className="flex h-full w-full flex-col">
			<Helmet>
				<title>Stump | {t('textSearch.title')}</title>
			</Helmet>
			<PageHeader>
				<PageHeader.Start>
					<Text className="font-medium">{t('textSearch.title')}</Text>
				</PageHeader.Start>
			</PageHeader>
			<div className="mx-auto flex w-full max-w-3xl flex-col gap-4 p-4">
				<form
					onSubmit={(event) => {
						event.preventDefault()
						const value = new FormData(event.currentTarget).get('q')?.toString().trim() ?? ''
						setParams(value ? { q: value } : {})
					}}
					className="flex gap-2"
				>
					<input
						key={q}
						name="q"
						defaultValue={q}
						autoFocus
						placeholder={t('textSearch.placeholder')}
						className="h-10 min-w-0 flex-1 rounded-md border border-border bg-background px-3 text-sm"
					/>
					<Button type="submit" variant="default">
						{t('textSearch.search')}
					</Button>
				</form>

				{!q && <Text variant="muted">{t('textSearch.hint')}</Text>}
				{q && isError && <Text variant="muted">{t('textSearch.unavailable')}</Text>}
				{q && data && !data.items.length && !isFetching && (
					<Text variant="muted">{t('textSearch.noMatches', { term: q })}</Text>
				)}
				{data && data.items.length > 0 && (
					<>
						<Text variant="muted" size="sm">
							{t('textSearch.total', { count: data.total })}
						</Text>
						<ul className="flex flex-col gap-3">
							{data.items.map((hit) => (
								<li
									key={`${hit.media_id}-${hit.section}-${hit.pos}`}
									className="rounded-md border border-border p-3"
								>
									<Link
										to={paths.bookOverview(hit.media_id)}
										className="font-medium hover:underline"
									>
										{hit.title}
									</Link>
									<div className="text-xs text-muted-foreground">
										{[
											hit.authors.join(', '),
											hit.section_title,
											hit.page_label ? t('textSearch.page', { label: hit.page_label }) : null,
										]
											.filter(Boolean)
											.join(' · ')}
									</div>
									{hit.snippet && (
										<p className="mt-2 text-sm text-foreground-muted">
											{highlight(hit.snippet, q)}
										</p>
									)}
								</li>
							))}
						</ul>
						{totalPages > 1 && (
							<div className="flex items-center justify-between">
								<Button variant="ghost" disabled={page <= 1} onClick={() => go(page - 1)}>
									{t('textSearch.previous')}
								</Button>
								<Text size="sm" variant="muted">
									{page} / {totalPages}
								</Text>
								<Button variant="ghost" disabled={page >= totalPages} onClick={() => go(page + 1)}>
									{t('textSearch.next')}
								</Button>
							</div>
						)}
					</>
				)}
			</div>
		</div>
	)
}
