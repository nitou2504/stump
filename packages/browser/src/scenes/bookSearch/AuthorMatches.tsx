import { useGraphQL } from '@stump/client'
import { graphql } from '@stump/graphql'

import { Link } from '@/context'
import { useTranslate } from '@/hooks/useTranslate'
import { usePaths } from '@/paths'

const query = graphql(`
	query BookSearchAuthors($search: String!) {
		searchAuthors(search: $search, limit: 6) {
			value
			count
		}
	}
`)

const MIN_LENGTH = 2

type Props = {
	search: string | undefined
}

/**
 * The authors a search names, above its books: each opens that author's books, with how many of
 * them the reader can see. Nothing is shown when no author answers.
 */
export default function AuthorMatches({ search }: Props) {
	const { t } = useTranslate()
	const paths = usePaths()
	const text = search?.trim() ?? ''

	const { data } = useGraphQL(
		query,
		['bookSearchAuthors', text],
		{ search: text },
		{ enabled: text.length >= MIN_LENGTH, staleTime: 30_000 },
	)
	const authors = text.length >= MIN_LENGTH ? (data?.searchAuthors ?? []) : []
	if (!authors.length) return null

	return (
		<section
			aria-label={t('globalSearch.authors')}
			className="px-4 pt-4 gap-2 flex flex-wrap items-center"
		>
			<span className="text-sm text-muted-foreground">{t('globalSearch.authors')}</span>
			{authors.map((author) => (
				<Link
					key={author.value}
					to={paths.bookSearchWithFilter({ metadata: { writers: { hasAnyOf: [author.value] } } })}
					className="gap-2 px-3 py-1 text-sm inline-flex items-center rounded-full border border-border bg-background hover:bg-muted"
				>
					<span>{author.value}</span>
					<span className="text-xs text-muted-foreground">
						{t('globalSearch.bookCount', { count: author.count })}
					</span>
				</Link>
			))}
		</section>
	)
}
