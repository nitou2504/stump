import { EBOOK_EXTENSION, PDF_EXTENSION, useGraphQLMutation, useSDK } from '@stump/client'
import { Button, ButtonOrLink, DropdownMenu } from '@stump/components'
import { FragmentType, graphql, useFragment } from '@stump/graphql'
import { Download, Ellipsis } from 'lucide-react'
import { useState } from 'react'
import { useNavigate } from 'react-router'
import { toast } from 'sonner'

import AddToReadingList from '@/components/readingList/AddToReadingList'
import { useTranslate } from '@/hooks/useTranslate'
import { usePaths } from '@/paths'

import { BookFormat, buildBookMoreMenu } from './bookMoreMenu'
import DeleteHistoryConfirmation from './DeleteHistoryConfirmation'
import EmailBookDialog from './EmailBookDialog'
import RemoveBookConfirmation from './RemoveBookConfirmation'
import { useBookPermissions } from './useBookPermissions'
import { useBookReadingActions } from './useBookReadingActions'
import { useBookThumbnailActions } from './useBookThumbnailActions'

const fragment = graphql(`
	fragment BookActions on Media {
		id
		resolvedName
		pages
		extension
		series {
			id
		}
		library {
			id
		}
		...BookReadingActions
	}
`)

const analyzeMutation = graphql(`
	mutation BookHeaderAnalyze($id: ID!) {
		analyzeMedia(id: $id)
	}
`)

type Props = {
	fragment: FragmentType<typeof fragment>
	/** Opens the sheet with the book's file and details */
	onShowInfo: () => void
	/**
	 * Opens the dialog to edit the book's details. The page owns that dialog, because more than one
	 * place on it opens it (the pencil by the genres too) and only one is ever mounted
	 */
	onEditDetails: () => void
}

const formatOf = (extension: string): BookFormat =>
	EBOOK_EXTENSION.test(extension) ? 'epub' : PDF_EXTENSION.test(extension) ? 'pdf' : 'comic'

/** The size of every icon button of the row: the same, and a finger's on touch screens */
const ICON_BUTTON = 'size-9 pointer-coarse:size-10'

// TODO(i8n): localize analysis

/**
 * What can be done to a book, under its cover: *Read* (or *Continue*) across the width, then three
 * icon buttons: *Download* (for who may), *Add to list* and *More*, which holds everything else
 * that the user may do. The same on a phone and on a desktop, for an EPUB, a PDF and a comic.
 *
 * The dialogs of these actions are mounted here, once. What the user can't do isn't shown
 * (see {@link useBookPermissions}): someone with no permission sees *Read*, *Add to list* and
 * *More*, and nothing else.
 */
export function BookActions({ fragment: fragmentRef, onShowInfo, onEditDetails }: Props) {
	const book = useFragment(fragment, fragmentRef)
	const navigate = useNavigate()
	const paths = usePaths()
	const { t, translate } = useTranslate()
	const { sdk } = useSDK()
	const permissions = useBookPermissions()
	const reading = useBookReadingActions(book)

	const thumbnail = useBookThumbnailActions({
		bookId: book.id,
		pages: book.pages,
		extension: book.extension,
	})
	const { mutate: analyze, isPending: isAnalyzing } = useGraphQLMutation(analyzeMutation, {
		onSuccess: () => toast.success('Analysis started'),
		onError: (error) => {
			console.error(error)
			toast.error('Failed to start analysis')
		},
	})

	const [showEmailDialog, setShowEmailDialog] = useState(false)
	const [showMarkAsUnreadConfirmation, setShowMarkAsUnreadConfirmation] = useState(false)
	const [showRemoveConfirmation, setShowRemoveConfirmation] = useState(false)

	const format = formatOf(book.extension)
	const { isReading, isPreviouslyCompleted, isUntouched, links } = reading

	const groups = buildBookMoreMenu({
		format,
		state: { isUntouched, isReading, isPreviouslyCompleted },
		permissions,
		canUploadCover: thumbnail.canUpload,
		links,
		isAnalyzing,
		translate,
		t,
		actions: {
			go: (to) => navigate(to),
			markAsRead: reading.completeBook,
			clearProgress: reading.deleteCurrentSession,
			askToMarkAsUnread: () => setShowMarkAsUnreadConfirmation(true),
			showFileInfo: onShowInfo,
			editDetails: onEditDetails,
			chooseCoverPage: thumbnail.openPageSelector,
			uploadCover: thumbnail.openUpload,
			analyze: () => analyze({ id: book.id }),
			send: () => setShowEmailDialog(true),
			remove: () => setShowRemoveConfirmation(true),
		},
	})

	const primaryLabel = isReading
		? translate('shared.common.continue')
		: isPreviouslyCompleted
			? translate('shared.common.readAgain')
			: translate('shared.common.read')
	const moreLabel = translate('shared.bookActions.more')
	const downloadLabel = translate('shared.common.download')

	return (
		<>
			<div className="gap-2 flex w-full flex-col">
				<ButtonOrLink href={isReading ? links.resume : links.fromBeginning} className="w-full">
					{primaryLabel}
				</ButtonOrLink>

				<div className="gap-2 flex items-center">
					{permissions.canDownload && (
						<Button
							variant="outline"
							size="icon"
							className={ICON_BUTTON}
							aria-label={downloadLabel}
							title={downloadLabel}
							onClick={() => downloadFile(sdk.media.downloadURL(book.id))}
						>
							<Download />
						</Button>
					)}

					<AddToReadingList bookId={book.id} bookName={book.resolvedName} />

					<DropdownMenu
						align="end"
						trigger={
							<Button
								variant="outline"
								size="icon"
								className={ICON_BUTTON}
								aria-label={moreLabel}
								title={moreLabel}
							>
								<Ellipsis />
							</Button>
						}
						groups={groups}
					/>
				</div>
			</div>

			{permissions.canEmail && (
				<EmailBookDialog
					mediaId={book.id}
					isOpen={showEmailDialog}
					onClose={() => setShowEmailDialog(false)}
				/>
			)}

			{permissions.canEditThumbnail && thumbnail.dialogs}

			{permissions.canRemove && (
				<RemoveBookConfirmation
					bookId={book.id}
					name={book.resolvedName}
					fallbackPath={
						book.series ? paths.seriesOverview(book.series.id) : paths.libraryBooks(book.library.id)
					}
					isOpen={showRemoveConfirmation}
					onClose={() => setShowRemoveConfirmation(false)}
				/>
			)}

			<DeleteHistoryConfirmation
				name={book.resolvedName}
				isOpen={showMarkAsUnreadConfirmation}
				onCancel={() => setShowMarkAsUnreadConfirmation(false)}
				onConfirm={() => {
					reading.deleteReadHistory()
					setShowMarkAsUnreadConfirmation(false)
				}}
			/>
		</>
	)
}

const downloadFile = (url: string) => {
	const anchor = document.createElement('a')
	anchor.href = url
	anchor.download = ''
	anchor.click()
}
