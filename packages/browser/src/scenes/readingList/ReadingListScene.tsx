import { useGraphQL } from '@stump/client'
import { ButtonOrLink, ConfirmationModal, Text } from '@stump/components'
import { graphql } from '@stump/graphql'
import { Book, PencilLine, Trash2, X } from 'lucide-react'
import { useState } from 'react'
import { Helmet } from 'react-helmet'
import { useParams } from 'react-router'
import { toast } from 'sonner'

import BookCard from '@/components/book/BookCard'
import { SceneContainer } from '@/components/container'
import DynamicCardGrid from '@/components/container/DynamicCardGrid'
import GenericEmptyState from '@/components/GenericEmptyState'
import { READING_LISTS_KEY, useReadingListActions } from '@/components/readingList/queries'
import { CollectionSceneHeader, EntityHeader } from '@/components/sharedLayout'
import { useAppContext, useNavigate } from '@/context'
import { useTranslate } from '@/hooks/useTranslate'
import { usePaths } from '@/paths'

import ReadingListFormDialog from './ReadingListFormDialog'

const query = graphql(`
	query ReadingListScene($id: ID!) {
		readingListById(id: $id) {
			id
			name
			description
			creatingUserId
			bookCount
			books {
				id
				resolvedName
				...BookCard
			}
		}
	}
`)

const errorMessage = (error: unknown) =>
	error instanceof Error && error.message ? error.message : undefined

/** A reading list: its books in the list's order; its owner can remove them, edit or delete it */
export default function ReadingListScene() {
	const { id = '' } = useParams()
	const paths = usePaths()
	const navigate = useNavigate()
	const { t } = useTranslate()
	const { user } = useAppContext()
	const {
		data,
		isLoading,
		error: loadError,
	} = useGraphQL(query, [READING_LISTS_KEY, 'list', id], { id })
	const { deleteList, addBook, removeBook, isDeleting } = useReadingListActions()

	const [isEditing, setIsEditing] = useState(false)
	const [isConfirmingDelete, setIsConfirmingDelete] = useState(false)

	const list = data?.readingListById
	const title = t('readingLists.title')

	// nothing until it's loaded, so the header doesn't change under the user
	if (isLoading) return null

	if (!list) {
		// e.g. "Reading list not found", or the server can't be reached
		return (
			<>
				<CollectionSceneHeader title={title} />
				<SceneContainer className="gap-4 items-center">
					<GenericEmptyState
						title={t('readingLists.errors.loadList')}
						subtitle={errorMessage(loadError)}
					/>
					<ButtonOrLink variant="outline" href={paths.readingLists()}>
						{t('readingLists.addToList.allLists')}
					</ButtonOrLink>
				</SceneContainer>
			</>
		)
	}

	const isOwner = list.creatingUserId === user.id

	const remove = async (book: { id: string; resolvedName: string }) => {
		try {
			await removeBook(list.id, book.id)
			toast.success(t('readingLists.list.removed', { name: book.resolvedName }), {
				action: {
					label: t('readingLists.list.undo'),
					onClick: () =>
						addBook(list.id, book.id).catch((error) =>
							toast.error(t('readingLists.errors.saveBook'), {
								description: errorMessage(error),
							}),
						),
				},
			})
		} catch (error) {
			toast.error(t('readingLists.errors.saveBook'), { description: errorMessage(error) })
		}
	}

	const confirmDelete = async () => {
		try {
			await deleteList(list.id)
			toast.success(t('readingLists.delete.deleted', { name: list.name }))
			navigate(paths.readingLists(), { replace: true })
		} catch (error) {
			toast.error(t('readingLists.errors.delete'), { description: errorMessage(error) })
		}
	}

	return (
		<>
			<Helmet>
				<title>Stump | {list.name}</title>
			</Helmet>

			<EntityHeader
				name={list.name}
				parents={[{ label: title, to: paths.readingLists() }]}
				tabs={[]}
				stats={[{ key: 'books', icon: Book, value: list.bookCount }]}
				actions={
					isOwner
						? [
								{
									items: [
										{
											label: t('readingLists.actions.edit'),
											leftIcon: <PencilLine />,
											onClick: () => setIsEditing(true),
										},
									],
								},
								{
									items: [
										{
											label: t('readingLists.actions.delete'),
											leftIcon: <Trash2 />,
											variant: 'destructive' as const,
											onClick: () => setIsConfirmingDelete(true),
										},
									],
								},
							]
						: undefined
				}
			/>

			<SceneContainer className="gap-4">
				{list.description && (
					<Text variant="muted" className="max-w-prose whitespace-pre-line">
						{list.description}
					</Text>
				)}

				{list.books.length ? (
					<DynamicCardGrid
						count={list.books.length}
						renderItem={(index) => {
							const book = list.books[index]
							if (!book) return null
							return (
								<div key={book.id} className="group/item relative">
									<BookCard fragment={book} />
									{isOwner && (
										<button
											type="button"
											onClick={() => remove(book)}
											aria-label={t('readingLists.list.removeBook', { name: book.resolvedName })}
											title={t('readingLists.list.removeBookShort')}
											// a 32px circle on touch screens, with a 44px area for the finger (a near miss
											// would open the book)
											className="left-2.5 top-2.5 size-7 pointer-coarse:size-8 bg-black/60 text-white backdrop-blur-sm hover:bg-black/80 after:-inset-1.5 absolute z-30 flex items-center justify-center rounded-full opacity-0 transition-opacity group-hover/item:opacity-100 after:absolute after:content-[''] focus-visible:opacity-100 focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none pointer-coarse:opacity-100"
										>
											<X className="size-4" />
										</button>
									)}
								</div>
							)
						}}
					/>
				) : (
					<GenericEmptyState
						title={t('readingLists.list.empty.heading')}
						subtitle={t('readingLists.list.empty.message')}
					/>
				)}
			</SceneContainer>

			{isOwner && (
				<>
					<ReadingListFormDialog
						list={list}
						isOpen={isEditing}
						onClose={() => setIsEditing(false)}
					/>
					<ConfirmationModal
						title={t('readingLists.delete.title')}
						description={t('readingLists.delete.description', { name: list.name })}
						isOpen={isConfirmingDelete}
						onClose={() => setIsConfirmingDelete(false)}
						onConfirm={confirmDelete}
						confirmIsLoading={isDeleting}
						confirmText={t('readingLists.delete.confirm')}
						confirmVariant="destructive"
					/>
				</>
			)}
		</>
	)
}
