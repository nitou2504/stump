import { useState } from 'react'
import { Helmet } from 'react-helmet'
import { useParams } from 'react-router'

import { useBookOverview } from '@/components/book'
import { BookActions } from '@/components/book/BookActions'
import { BookHeader } from '@/components/book/BookHeader'
import { BookInfoSheet } from '@/components/book/BookInfoSheet'
import { BookOverviewDetails } from '@/components/book/BookOverviewDetails'
import { EditBookDialog } from '@/components/book/metadata/edit'
import { useBookPermissions } from '@/components/book/useBookPermissions'
import { SceneContainer } from '@/components/container'
import { ProminentThumbnailImage } from '@/components/thumbnail'

import BooksInSeries from './BooksInSeries'
import MoreByAuthor from './MoreByAuthor'

export default function BookOverviewScene() {
	const { id } = useParams()
	const {
		data: { mediaById: media },
	} = useBookOverview(id || '')
	const { canEditMetadata } = useBookPermissions()

	if (!media) {
		throw new Error('Book not found')
	}

	const [isInfoSheetOpen, setIsInfoSheetOpen] = useState(false)
	// The dialog to edit the book's details is mounted once, here: the *More* menu and the pencil by
	// the genres both open it, the pencil with the genres in view (`undefined` is closed)
	const [editing, setEditing] = useState<{ focus?: 'genres' } | undefined>(undefined)

	return (
		<>
			<Helmet>
				<title>Stump | {media.resolvedName}</title>
			</Helmet>

			<BookHeader fragment={media} />

			<SceneContainer className="gap-8 @container">
				<div className="gap-6 @xl:grid-cols-[13rem_minmax(0,1fr)] grid grid-cols-1">
					<div className="gap-3 @xl:max-w-none max-w-56 mx-auto flex w-full flex-col">
						<ProminentThumbnailImage
							src={media.thumbnail.url}
							alt={media.resolvedName}
							placeholderData={media.thumbnail.metadata}
						/>
						<BookActions
							fragment={media}
							onShowInfo={() => setIsInfoSheetOpen(true)}
							onEditDetails={() => setEditing({})}
						/>
					</div>

					<BookOverviewDetails
						fragment={media}
						onEditGenres={canEditMetadata ? () => setEditing({ focus: 'genres' }) : undefined}
					/>
				</div>

				{media.seriesId && (
					// remounted per book so the row starts at it (keys must differ from MoreByAuthor's)
					<BooksInSeries key={`series-${media.id}`} bookId={media.id} seriesId={media.seriesId} />
				)}

				{!!media.metadata?.writers?.[0] && (
					<MoreByAuthor
						key={media.id}
						bookId={media.id}
						seriesId={media.seriesId}
						author={media.metadata.writers[0]}
					/>
				)}
			</SceneContainer>

			<BookInfoSheet
				fragment={media}
				isOpen={isInfoSheetOpen}
				onClose={() => setIsInfoSheetOpen(false)}
			/>

			{canEditMetadata && (
				<EditBookDialog
					mediaId={media.id}
					isOpen={!!editing}
					onClose={() => setEditing(undefined)}
					focus={editing?.focus}
				/>
			)}
		</>
	)
}
