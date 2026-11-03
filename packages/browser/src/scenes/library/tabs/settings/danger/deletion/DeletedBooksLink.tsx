import { useGraphQL } from '@stump/client'
import { ButtonOrLink, Heading, Text } from '@stump/components'
import { graphql } from '@stump/graphql'
import { ArrowRight } from 'lucide-react'

import { usePaths } from '@/paths'

import { useLibraryManagement } from '../../context'

const query = graphql(`
	query LibraryDeletedBooks($libraryId: ID!) {
		deletedBooks(libraryId: $libraryId) {
			book {
				id
			}
		}
		deletedBooksSettings {
			retentionDays
		}
	}
`)

/**
 * Books removed from this library wait in Settings → Deleted books, opened here on this
 * library's books
 */
export default function DeletedBooksLink() {
	const {
		library: { id },
	} = useLibraryManagement()
	const paths = usePaths()
	const { data } = useGraphQL(query, ['deletedBooks', 'library', id], { libraryId: id })
	const count = data?.deletedBooks.length ?? 0
	const days = data?.deletedBooksSettings.retentionDays

	return (
		<div className="gap-4 flex flex-col">
			<div>
				<Heading size="sm">Deleted books</Heading>
				<Text size="sm" variant="muted">
					{days === undefined
						? '\u00a0'
						: days === 0
							? 'Removed books wait there until you delete them.'
							: `Removed books wait there ${days === 1 ? '1 day' : `${days} days`}.`}
				</Text>
			</div>
			<div>
				<ButtonOrLink variant="outline" href={paths.deletedBooks(id)}>
					{count === 0
						? 'Open Deleted books'
						: `Review ${count === 1 ? '1 deleted book' : `${count} deleted books`}`}
					<ArrowRight aria-hidden />
				</ButtonOrLink>
			</div>
		</div>
	)
}
