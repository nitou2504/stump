import { useGraphQL } from '@stump/client'
import { ButtonOrLink } from '@stump/components'
import {
	graphql,
	MediaMetadataModelOrdering,
	MediaModelOrdering,
	MediaOrderBy,
	OrderDirection,
	ReadingStatus,
} from '@stump/graphql'
import { Play } from 'lucide-react'

import {
	BookReadingActionsRef,
	useBookReadingActions,
} from '@/components/book/useBookReadingActions'
import { useTranslate } from '@/hooks/useTranslate'

const query = graphql(`
	query SeriesContinue($id: ID!, $inProgress: MediaFilterInput!, $orderBy: [MediaOrderBy!]!) {
		seriesById(id: $id) {
			id
			percentageCompleted
			upNext(take: 1) {
				id
				resolvedName
				...BookReadingActions
			}
		}
		media(
			filter: $inProgress
			orderBy: $orderBy
			pagination: { offset: { page: 1, pageSize: 1 } }
		) {
			nodes {
				id
				resolvedName
				...BookReadingActions
			}
		}
	}
`)

/**
 * The series' books in the order `Series.upNext` lists them (number, then name), so that of
 * several books in progress the first one in the series is the one asked for
 */
const SERIES_ORDER: MediaOrderBy[] = [
	{ metadata: { field: MediaMetadataModelOrdering.Number, direction: OrderDirection.Asc } },
	{ media: { field: MediaModelOrdering.Name, direction: OrderDirection.Asc } },
]

type Props = {
	seriesId: string
	/**
	 * Where it sits: in the page header from a small screen up, or as a full-width bar at the top of
	 * the list of books on a phone (the header has no room for it there: the series name needs it)
	 */
	placement?: 'header' | 'bar'
}

/**
 * *Continue* on a series (*Start reading* when nothing of it is read yet): opens the book of the
 * series the reader is in the middle of, at the page they were on, else the next one they haven't
 * finished. A book in progress comes first even when an earlier book is still unread (someone who
 * started with book 2 continues book 2). Nothing when every book is read, or when the series has no
 * book the reader can see.
 */
export function ContinueSeriesButton({ seriesId, placement = 'header' }: Props) {
	const { data } = useGraphQL(query, ['seriesContinue', seriesId], {
		id: seriesId,
		inProgress: { seriesId: { eq: seriesId }, readingStatus: { is: ReadingStatus.Reading } },
		orderBy: SERIES_ORDER,
	})

	const series = data?.seriesById
	const next = data?.media?.nodes?.[0] ?? series?.upNext?.[0]
	if (!series || !next) return null

	return (
		<ContinueLink
			book={next}
			title={next.resolvedName}
			started={(series.percentageCompleted ?? 0) > 0}
			placement={placement}
		/>
	)
}

function ContinueLink({
	book,
	title,
	started,
	placement,
}: {
	book: BookReadingActionsRef
	title: string
	started: boolean
	placement: NonNullable<Props['placement']>
}) {
	const { t, translate } = useTranslate()
	const { isReading, links } = useBookReadingActions(book)

	const isContinuing = isReading || started

	const link = (
		<ButtonOrLink
			href={links.resume}
			size="sm"
			className={placement === 'bar' ? 'w-full' : 'shrink-0'}
			title={t(isContinuing ? 'seriesContinue.continueWith' : 'seriesContinue.startWith', {
				title,
			})}
		>
			<Play className="h-3.5 w-3.5" />
			{isContinuing ? translate('shared.common.continue') : t('seriesContinue.start')}
		</ButtonOrLink>
	)

	return placement === 'bar' ? (
		<div className="px-4 pt-3 sm:hidden">{link}</div>
	) : (
		<div className="sm:block hidden shrink-0">{link}</div>
	)
}
