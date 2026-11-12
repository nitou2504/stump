import { useGraphQL, useSDK } from '@stump/client'
import { Avatar, Button, ButtonOrLink, ConfirmationModal, Text } from '@stump/components'
import { Book, Copy, Globe, GlobeLock, PencilLine, Trash2, X } from 'lucide-react'
import { useMemo, useState } from 'react'
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
import { useStatLabels } from '@/hooks/useStatLabels'
import { useTranslate } from '@/hooks/useTranslate'
import { usePaths } from '@/paths'

import { fetchReadingListBookIds, readingListQuery, useReadingListBooks } from './graphql'
import ReadingListFormDialog from './ReadingListFormDialog'

const errorMessage = (error: unknown) =>
	error instanceof Error && error.message ? error.message : undefined

/** What the server accepts for a copy's name and description from someone who isn't its owner */
const MAX_NAME = 200
const MAX_DESCRIPTION = 2000

/**
 * A reading list: its books in the list's order, a page at a time. Its maker can remove books,
 * edit or delete it. Anyone else sees it read-only and can save a copy of it to their own lists;
 * the server owner can also take someone else's public list back to private, or delete it.
 */
export default function ReadingListScene() {
	const { id = '' } = useParams()
	const paths = usePaths()
	const navigate = useNavigate()
	const { sdk } = useSDK()
	const { t } = useTranslate()
	const labels = useStatLabels()
	const { user } = useAppContext()
	const {
		data,
		isLoading,
		error: loadError,
	} = useGraphQL(readingListQuery, [READING_LISTS_KEY, 'list', id], { id })
	const {
		data: booksData,
		isLoading: isLoadingBooks,
		hasNextPage,
		fetchNextPage,
		isFetchingNextPage,
	} = useReadingListBooks(id)
	const { createList, deleteList, unpublishList, addBook, removeBook, isDeleting } =
		useReadingListActions()

	const [isEditing, setIsEditing] = useState(false)
	const [isConfirmingDelete, setIsConfirmingDelete] = useState(false)
	const [isConfirmingUnpublish, setIsConfirmingUnpublish] = useState(false)
	const [isCopying, setIsCopying] = useState(false)

	const list = data?.readingListById
	const books = useMemo(
		() => booksData?.pages.flatMap((page) => page.readingListById.books) ?? [],
		[booksData],
	)
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

	const canEdit = list.canEdit
	// the server owner can't change someone else's list, only take it back from the public
	const isModerator = !canEdit && user.isServerOwner
	const isPublic = list.visibility === 'PUBLIC'
	const creatorName = list.creator?.username

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

	const confirmUnpublish = async () => {
		try {
			await unpublishList(list)
			toast.success(t('readingLists.unpublish.done', { name: list.name }))
			setIsConfirmingUnpublish(false)
			// it is private now, and not the server owner's to see
			navigate(paths.readingLists(), { replace: true })
		} catch (error) {
			toast.error(t('readingLists.errors.unpublish'), { description: errorMessage(error) })
		}
	}

	/** A private copy of the list, with the books the user can see, in their own lists */
	const saveCopy = async () => {
		setIsCopying(true)
		try {
			const mediaIds = await fetchReadingListBookIds(sdk, list.id)
			const copy = await createList({
				name: t('readingLists.copy.name', { name: list.name }).slice(0, MAX_NAME),
				description: list.description?.slice(0, MAX_DESCRIPTION),
				mediaIds,
			})
			toast.success(t('readingLists.copy.saved', { name: copy.name }))
			navigate(paths.readingList(copy.id))
		} catch (error) {
			toast.error(t('readingLists.errors.copy'), { description: errorMessage(error) })
		} finally {
			setIsCopying(false)
		}
	}

	const saveCopyAction = {
		label: t('readingLists.actions.saveCopy'),
		leftIcon: <Copy />,
		onClick: saveCopy,
	}

	const actions = canEdit
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
		: [
				{ items: [saveCopyAction] },
				...(isModerator
					? [
							{
								items: [
									{
										label: t('readingLists.actions.unpublish'),
										leftIcon: <GlobeLock />,
										onClick: () => setIsConfirmingUnpublish(true),
									},
									{
										label: t('readingLists.actions.delete'),
										leftIcon: <Trash2 />,
										variant: 'destructive' as const,
										onClick: () => setIsConfirmingDelete(true),
									},
								],
							},
						]
					: []),
			]

	return (
		<>
			<Helmet>
				<title>Stump | {list.name}</title>
			</Helmet>

			<EntityHeader
				name={list.name}
				parents={[{ label: title, to: paths.readingLists() }]}
				tabs={[]}
				stats={[
					{
						key: 'books',
						icon: Book,
						value: labels.number(list.bookCount),
						suffix: labels.word('books', list.bookCount),
					},
				]}
				actions={actions}
				endActions={
					!canEdit && (
						<Button
							variant="outline"
							size="sm"
							onClick={saveCopy}
							disabled={isCopying}
							className="sm:inline-flex hidden"
						>
							<Copy />
							{t('readingLists.actions.saveCopy')}
						</Button>
					)
				}
			/>

			<SceneContainer className="gap-4">
				{isPublic && (
					<div className="gap-2 text-sm flex items-center text-muted-foreground">
						<Globe className="size-4 shrink-0" />
						{canEdit ? (
							<span>{t('readingLists.list.publicYours')}</span>
						) : (
							<span className="gap-2 flex items-center">
								{t('readingLists.list.publicBy', { name: creatorName })}
								{list.creator && (
									<Avatar
										className="size-5"
										src={list.creator.avatarUrl ?? undefined}
										fallback={list.creator.username}
									/>
								)}
							</span>
						)}
					</div>
				)}

				{list.description && (
					<Text variant="muted" className="max-w-prose whitespace-pre-line">
						{list.description}
					</Text>
				)}

				{books.length ? (
					<>
						<DynamicCardGrid
							count={books.length}
							renderItem={(index) => {
								const book = books[index]
								if (!book) return null
								return (
									<div key={book.id} className="group/item relative">
										<BookCard fragment={book} />
										{canEdit && (
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
						{hasNextPage && (
							<div className="gap-2 flex flex-col items-center">
								<Text variant="muted" size="sm">
									{t('readingLists.list.shown', { shown: books.length, total: list.bookCount })}
								</Text>
								<Button
									variant="outline"
									onClick={() => fetchNextPage()}
									disabled={isFetchingNextPage}
								>
									{t('readingLists.list.loadMore')}
								</Button>
							</div>
						)}
					</>
				) : (
					!isLoadingBooks && (
						<GenericEmptyState
							title={t('readingLists.list.empty.heading')}
							subtitle={
								canEdit
									? t('readingLists.list.empty.message')
									: t('readingLists.list.empty.messageOthers')
							}
						/>
					)
				)}
			</SceneContainer>

			{(canEdit || isModerator) && (
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
			)}

			{canEdit && (
				<ReadingListFormDialog list={list} isOpen={isEditing} onClose={() => setIsEditing(false)} />
			)}

			{isModerator && (
				<ConfirmationModal
					title={t('readingLists.unpublish.title')}
					description={t('readingLists.unpublish.description', {
						name: list.name,
						creator: creatorName,
					})}
					isOpen={isConfirmingUnpublish}
					onClose={() => setIsConfirmingUnpublish(false)}
					onConfirm={confirmUnpublish}
					confirmText={t('readingLists.unpublish.confirm')}
				/>
			)}
		</>
	)
}
