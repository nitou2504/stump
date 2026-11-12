import { useGraphQL } from '@stump/client'
import { ButtonOrLink } from '@stump/components'
import { graphql } from '@stump/graphql'
import { Play } from 'lucide-react'

import {
	BookReadingActionsRef,
	useBookReadingActions,
} from '@/components/book/useBookReadingActions'
import { useTranslate } from '@/hooks/useTranslate'

const query = graphql(`
	query SeriesContinue($id: ID!) {
		seriesById(id: $id) {
			id
			percentageCompleted
			upNext(take: 1) {
				id
				resolvedName
				...BookReadingActions
			}
		}
	}
`)

type Props = {
	seriesId: string
}

/**
 * *Continue* on a series (*Start reading* when nothing of it is read yet): opens the next book of
 * the series that the reader hasn't finished, at the page they were on. Nothing when every book is
 * read, or when the series has no book the reader can see.
 */
export function ContinueSeriesButton({ seriesId }: Props) {
	const { data } = useGraphQL(query, ['seriesContinue', seriesId], { id: seriesId })

	const series = data?.seriesById
	const next = series?.upNext?.[0]
	if (!series || !next) return null

	return (
		<ContinueLink
			book={next}
			title={next.resolvedName}
			started={(series.percentageCompleted ?? 0) > 0}
		/>
	)
}

function ContinueLink({
	book,
	title,
	started,
}: {
	book: BookReadingActionsRef
	title: string
	started: boolean
}) {
	const { t, translate } = useTranslate()
	const { isReading, links } = useBookReadingActions(book)

	const isContinuing = isReading || started

	return (
		<ButtonOrLink
			href={links.resume}
			size="sm"
			className="shrink-0"
			title={t(isContinuing ? 'seriesContinue.continueWith' : 'seriesContinue.startWith', {
				title,
			})}
		>
			<Play className="h-3.5 w-3.5" />
			{isContinuing ? translate('shared.common.continue') : t('seriesContinue.start')}
		</ButtonOrLink>
	)
}
