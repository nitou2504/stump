import { FragmentType, graphql, useFragment } from '@stump/graphql'
import { Book, Rows3 } from 'lucide-react'

import { CollectionGridItem } from '@/components/collection'
import { useStatLabels } from '@/hooks/useStatLabels'
import { usePaths } from '@/paths'

const fragment = graphql(`
	fragment LibraryWebGridItem on Library {
		id
		name
		series(take: 5) {
			thumbnail {
				url
				metadata {
					averageColor
					colors {
						color
						percentage
					}
					thumbhash
				}
			}
		}
		stats {
			seriesCount
			bookCount
		}
	}
`)

type Props = {
	data: FragmentType<typeof fragment>
	getLayoutNumber: (id: string, itemCount: number) => number | undefined
}

export function LibraryGridItem({ data, getLayoutNumber }: Props) {
	const paths = usePaths()
	const labels = useStatLabels()
	const { id, name, series, stats } = useFragment(fragment, data)

	const layoutNumber = getLayoutNumber(id, series.length)

	return (
		<CollectionGridItem
			title={name}
			to={paths.library(id)}
			thumbnails={series.map(({ thumbnail }) => ({
				url: thumbnail.url,
				metadata: thumbnail.metadata,
			}))}
			layoutNumber={layoutNumber}
			stats={[
				{
					key: 'series',
					icon: Rows3,
					value: labels.number(stats.seriesCount),
					unit: labels.word('series', stats.seriesCount),
					label: labels.count('series', stats.seriesCount),
				},
				{
					key: 'books',
					icon: Book,
					value: labels.number(stats.bookCount),
					unit: labels.word('books', stats.bookCount),
					label: labels.count('books', stats.bookCount),
				},
			]}
		/>
	)
}
