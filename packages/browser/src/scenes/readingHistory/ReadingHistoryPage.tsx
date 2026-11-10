import { Helmet } from 'react-helmet'

import { SceneContainer } from '@/components/container'
import GenericEmptyState from '@/components/GenericEmptyState'
import { CollectionSceneHeader } from '@/components/sharedLayout'
import { useTranslate } from '@/hooks/useTranslate'

import FinishedSection from './FinishedSection'
import { useReader, useReadingPeople } from './graphql'
import ReaderSwitcher from './ReaderSwitcher'
import ReadingNowSection from './ReadingNowSection'
import { ReadingPrivacyControl } from './ReadingPrivacy'

type Props = {
	/** A friend's id (`/reading/:userId`), or nothing for the person's own page (`/reading`) */
	userId?: string
}

/**
 * What a person has finished and is reading now: the page of the person asking, or, when they let
 * everyone see it, of a friend. A friend's page is read-only and shows what the person asking
 * may read of it. A person who doesn't exist, doesn't share or was deleted is the same page:
 * "this isn't available", so it can't be told which of them it was.
 */
export default function ReadingHistoryPage({ userId }: Props) {
	const { t } = useTranslate()
	const { data: reader, error } = useReader(userId)
	const { data: people } = useReadingPeople()

	const isOwnPage = !userId || !!reader?.readingHistory.isSelf
	const name = reader?.readingHistory.reader.username
	const title =
		isOwnPage || !name ? t('readingHistory.title') : t('readingHistory.titleOf', { name })

	return (
		<>
			<Helmet>
				<title>Stump | {t('readingHistory.helmet')}</title>
			</Helmet>

			<CollectionSceneHeader
				title={title}
				// who sees the page is the person's to change, on their own page only; it is a chip in the
				// header so the page itself starts with the reading
				endActions={isOwnPage ? <ReadingPrivacyControl /> : undefined}
			/>

			<SceneContainer className="gap-8">
				<ReaderSwitcher
					friends={people?.readersSharingHistory ?? []}
					current={isOwnPage ? undefined : userId}
				/>

				{error ? (
					<GenericEmptyState
						title={t('readingHistory.unavailable.title')}
						subtitle={t('readingHistory.unavailable.description')}
					/>
				) : (
					reader && (
						<>
							<ReadingNowSection userId={userId} />
							<FinishedSection userId={userId} />
						</>
					)
				)}
			</SceneContainer>
		</>
	)
}
