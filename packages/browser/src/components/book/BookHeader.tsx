import { FragmentType, graphql, useFragment } from '@stump/graphql'
import { useLocation } from 'react-router'

import { EntityHeader } from '@/components/sharedLayout'
import { useTranslate } from '@/hooks/useTranslate'
import { usePaths } from '@/paths'

import { useBookPermissions } from './useBookPermissions'

const fragment = graphql(`
	fragment BookHeader on Media {
		id
		resolvedName
		series {
			id
			resolvedName
		}
		library {
			id
			name
		}
	}
`)

type Props = {
	fragment: FragmentType<typeof fragment>
}

/**
 * The title bar of a book's pages: where the book is (library, series) and the way to its manage
 * page, for the users that page has something for. What can be done to the book is under its
 * cover (see `BookActions`), the same for everyone.
 */
export function BookHeader({ fragment: fragmentRef }: Props) {
	const book = useFragment(fragment, fragmentRef)
	const location = useLocation()
	const paths = usePaths()

	const { translate } = useTranslate()
	const { canManage } = useBookPermissions()

	const tabs = [
		{
			isActive: location.pathname === paths.bookOverview(book.id),
			label: translate('shared.common.overview'),
			to: paths.bookOverview(book.id),
		},
		// The reading timeline page is not implemented on web yet, so it is not linked here
	]

	return (
		<EntityHeader
			name={book.resolvedName}
			parents={[
				{
					label: book.library.name,
					to: book.series ? paths.library(book.library.id) : paths.libraryBooks(book.library.id),
				},
				...(book.series
					? [{ label: book.series.resolvedName, to: paths.seriesOverview(book.series.id) }]
					: []),
			]}
			tabs={tabs}
			settingsLink={canManage ? paths.bookManagement(book.id) : undefined}
		/>
	)
}
