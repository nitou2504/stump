import { Text } from '@stump/components'

import { ThumbnailImage } from '@/components/thumbnail'
import { Link } from '@/context'
import { useTranslate } from '@/hooks/useTranslate'
import { usePaths } from '@/paths'

import { formatDay } from './dates'
import { useReadingNow } from './graphql'

type Props = {
	/** A friend's id, or nothing for the person's own page */
	userId?: string
}

/**
 * The books the reader is reading now, the one read last first, with how far along they are. A
 * friend's page shows the friend's progress: it comes from them, never from the book's own
 * progress, which is always the viewer's
 */
export default function ReadingNowSection({ userId }: Props) {
	const { t } = useTranslate()
	const paths = usePaths()
	const { data, isLoading } = useReadingNow(userId)

	if (isLoading) return null

	const reading = data?.readingHistory.reading ?? []

	return (
		<section aria-label={t('readingHistory.now.heading')} className="gap-3 flex flex-col">
			<h2 className="text-lg font-semibold">{t('readingHistory.now.heading')}</h2>

			{reading.length ? (
				<ul className="gap-4 sm:grid-cols-3 md:grid-cols-4 xl:grid-cols-6 grid grid-cols-2">
					{reading.map(({ media, percentage, lastReadOn }) => (
						<li key={media.id}>
							<Link
								to={paths.bookOverview(media.id)}
								className="gap-2 flex flex-col rounded-thumbnail hover:opacity-90"
							>
								<ThumbnailImage
									src={media.thumbnail.url}
									alt={media.resolvedName}
									size={{ width: '100%', height: 'auto' }}
									className="aspect-2/3"
									lazy
								/>
								<span className="gap-0.5 flex flex-col">
									<Text className="text-sm font-medium leading-tight line-clamp-2">
										{media.resolvedName}
									</Text>
									{percentage != null && (
										<span className="gap-2 flex items-center">
											<span
												role="progressbar"
												aria-label={t('readingHistory.now.progress', { percent: percentage })}
												aria-valuenow={percentage}
												aria-valuemin={0}
												aria-valuemax={100}
												className="h-1 flex-1 overflow-hidden rounded-full bg-muted"
											>
												<span
													className="block h-full rounded-full bg-primary"
													style={{ width: `${percentage}%` }}
												/>
											</span>
											<Text variant="muted" size="xs">
												{percentage}%
											</Text>
										</span>
									)}
									<Text variant="muted" size="xs">
										{t('readingHistory.now.lastRead', { date: formatDay(lastReadOn) })}
									</Text>
								</span>
							</Link>
						</li>
					))}
				</ul>
			) : (
				<Text variant="muted">{t('readingHistory.now.empty')}</Text>
			)}
		</section>
	)
}
