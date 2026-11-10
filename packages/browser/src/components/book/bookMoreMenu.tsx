import { DropdownItem, DropdownItemGroup } from '@stump/components/dropdown/DropdownMenu'
import {
	BookMinus,
	BookOpen,
	BookOpenCheck,
	BookX,
	EyeOff,
	FileSearch,
	FileText,
	Image,
	ImagePlus,
	ImageUp,
	Info,
	PencilLine,
	Send,
	Trash2,
} from 'lucide-react'

import type { BookPermissions } from './useBookPermissions'

export type BookFormat = 'epub' | 'pdf' | 'comic'

export type BookMoreMenuContext = {
	format: BookFormat
	/** Where the user is in the book */
	state: { isUntouched: boolean; isReading: boolean; isPreviouslyCompleted: boolean }
	permissions: BookPermissions
	/** The server has uploads on and the user may upload, so a cover can be an image they send */
	canUploadCover: boolean
	links: { fromBeginning: string; incognito: string; nativePdf?: string }
	isAnalyzing: boolean
	/** A label from its key under the locale root (`shared.…`) and one from the web app's own keys */
	translate: (key: string) => string
	t: (key: string) => string
	actions: {
		go: (to: string) => void
		markAsRead: () => void
		clearProgress: () => void
		/** Asks first: it deletes the history of the book's completions */
		askToMarkAsUnread: () => void
		showFileInfo: () => void
		editDetails: () => void
		chooseCoverPage: () => void
		uploadCover: () => void
		analyze: () => void
		send: () => void
		remove: () => void
	}
}

/** These two were English text before the menu was rebuilt; they stay as they were */
export const EDIT_DETAILS_LABEL = 'Edit details'
export const REMOVE_FROM_LIBRARY_LABEL = 'Remove from library'

/**
 * The *More* menu of a book's page: what is left of the book's actions after *Read*,
 * *Download* and *Add to list*, which are the buttons under the cover. Four groups, in this
 * order and with what the user can't do left out (a group with nothing in it is left out too):
 *
 * 1. **Reading**: from the beginning (for a book already begun or finished), incognito, mark as
 *    read, reset progress, mark as unread
 * 2. **Get**: the browser's PDF viewer and email
 * 3. **Book**: file info, edit details, the cover (pick a page, for an EPUB its own cover, or upload
 *    an image), analyze (not for an EPUB: it has no pages to measure)
 * 4. **Remove from library**, last because it is the one that can't be taken back by a click
 *
 * The reading items need no permission, so the menu is never empty: someone with no permission at
 * all still has *Read incognito* and the file info.
 */
export function buildBookMoreMenu(ctx: BookMoreMenuContext): DropdownItemGroup[] {
	const { format, state, permissions, links, actions, translate, t } = ctx

	const reading: DropdownItem[] = [
		...(state.isReading || state.isPreviouslyCompleted
			? [
					{
						label: translate('shared.bookActions.readFromBeginning'),
						leftIcon: <BookOpen />,
						onClick: () => actions.go(links.fromBeginning),
					},
				]
			: []),
		{
			label: translate('shared.bookActions.readIncognito'),
			leftIcon: <EyeOff />,
			onClick: () => actions.go(links.incognito),
		},
		...(state.isUntouched || state.isReading
			? [
					{
						label: translate('shared.bookActions.markAsRead.label'),
						leftIcon: <BookOpenCheck />,
						onClick: actions.markAsRead,
					},
				]
			: []),
		...(state.isReading
			? [
					{
						label: translate('shared.bookActions.clearProgress.label'),
						leftIcon: <BookMinus />,
						onClick: actions.clearProgress,
					},
				]
			: []),
		...(state.isPreviouslyCompleted
			? [
					{
						label: translate('shared.bookActions.markAsUnread'),
						leftIcon: <BookX />,
						variant: 'destructive' as const,
						onClick: actions.askToMarkAsUnread,
					},
				]
			: []),
	]

	const nativePdf = links.nativePdf
	const get: DropdownItem[] = [
		// the browser's viewer fetches the original file, which is what *Download* is for
		...(format === 'pdf' && permissions.canDownload && nativePdf
			? [
					{
						label: translate('shared.bookActions.openNativePdf'),
						leftIcon: <FileText />,
						onClick: () => actions.go(nativePdf),
					},
				]
			: []),
		...(permissions.canEmail
			? [
					{
						label: translate('shared.common.email'),
						leftIcon: <Send />,
						onClick: actions.send,
					},
				]
			: []),
	]

	// picking a page is for every format: an EPUB has one to pick, its own cover (the page grid and the
	// server both stop at page 1), which is how a user who may edit thumbnails but not upload files puts
	// the book's cover back after an upload
	const coverItems: DropdownItem[] = [
		{
			label: t('thumbnailSelector.chooseAPage'),
			leftIcon: <ImagePlus />,
			onClick: actions.chooseCoverPage,
		},
		...(ctx.canUploadCover
			? [
					{
						label: t('thumbnailSelector.uploadImage'),
						leftIcon: <ImageUp />,
						onClick: actions.uploadCover,
					},
				]
			: []),
	]

	const book: DropdownItem[] = [
		{
			label: translate('shared.bookActions.fileInfo'),
			leftIcon: <Info />,
			onClick: actions.showFileInfo,
		},
		...(permissions.canEditMetadata
			? [
					{
						label: EDIT_DETAILS_LABEL,
						leftIcon: <PencilLine />,
						onClick: actions.editDetails,
					},
				]
			: []),
		...(permissions.canEditThumbnail
			? [
					{
						label: translate('shared.common.thumbnail'),
						leftIcon: <Image />,
						subItems: coverItems,
					},
				]
			: []),
		...(permissions.canAnalyze && format !== 'epub'
			? [
					{
						label: translate('shared.bookActions.analyze'),
						leftIcon: <FileSearch />,
						disabled: ctx.isAnalyzing,
						onClick: actions.analyze,
					},
				]
			: []),
	]

	const remove: DropdownItem[] = permissions.canRemove
		? [
				{
					label: REMOVE_FROM_LIBRARY_LABEL,
					leftIcon: <Trash2 />,
					variant: 'destructive' as const,
					onClick: actions.remove,
				},
			]
		: []

	return [{ items: reading }, { items: get }, { items: book }, { items: remove }].filter(
		(group) => group.items.length > 0,
	)
}
