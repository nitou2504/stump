import { UserPermission } from '@stump/graphql'

import {
	BookFormat,
	buildBookMoreMenu,
	EDIT_DETAILS_LABEL,
	REMOVE_FROM_LIBRARY_LABEL,
} from '../bookMoreMenu'
import { pickBookPermissions } from '../useBookPermissions'

type Person = { permissions: UserPermission[]; canUploadCover?: boolean }

type PersonKey = 'friend' | 'friend who may download' | 'editor' | 'manager' | 'remover' | 'owner'

const PEOPLE: Record<PersonKey, Person> = {
	friend: { permissions: [] },
	'friend who may download': { permissions: [UserPermission.DownloadFile] },
	editor: {
		permissions: [UserPermission.EditMetadata, UserPermission.EditThumbnails],
		canUploadCover: true,
	},
	manager: {
		permissions: [
			UserPermission.DownloadFile,
			UserPermission.ManageLibrary,
			UserPermission.EditMetadata,
			UserPermission.EditThumbnails,
			UserPermission.MetadataFetchRecordManage,
		],
	},
	remover: { permissions: [UserPermission.RemoveBook] },
	owner: { permissions: Object.values(UserPermission), canUploadCover: true },
}

const FORMATS: BookFormat[] = ['epub', 'pdf', 'comic']

const STATES = {
	untouched: { isUntouched: true, isReading: false, isPreviouslyCompleted: false },
	reading: { isUntouched: false, isReading: true, isPreviouslyCompleted: false },
	completed: { isUntouched: false, isReading: false, isPreviouslyCompleted: true },
}

const KEY = {
	fromBeginning: 'shared.bookActions.readFromBeginning',
	incognito: 'shared.bookActions.readIncognito',
	markAsRead: 'shared.bookActions.markAsRead.label',
	clearProgress: 'shared.bookActions.clearProgress.label',
	markAsUnread: 'shared.bookActions.markAsUnread',
	nativePdf: 'shared.bookActions.openNativePdf',
	email: 'shared.common.email',
	fileInfo: 'shared.bookActions.fileInfo',
	cover: 'shared.common.thumbnail',
	choosePage: 'thumbnailSelector.chooseAPage',
	upload: 'thumbnailSelector.uploadImage',
	analyze: 'shared.bookActions.analyze',
}

const noop = () => {}

function menuFor(
	personKey: PersonKey,
	format: BookFormat,
	state: keyof typeof STATES,
	overrides: { isAnalyzing?: boolean } = {},
) {
	const person = PEOPLE[personKey]
	return buildBookMoreMenu({
		format,
		state: STATES[state],
		permissions: pickBookPermissions((permission) => person.permissions.includes(permission)),
		canUploadCover: !!person.canUploadCover,
		links: {
			fromBeginning: '/from-the-start',
			incognito: '/incognito',
			nativePdf: format === 'pdf' ? '/browser-viewer' : undefined,
		},
		isAnalyzing: overrides.isAnalyzing ?? false,
		translate: (key) => key,
		t: (key) => key,
		actions: {
			go: noop,
			markAsRead: noop,
			clearProgress: noop,
			askToMarkAsUnread: noop,
			showFileInfo: noop,
			editDetails: noop,
			chooseCoverPage: noop,
			uploadCover: noop,
			analyze: noop,
			send: noop,
			remove: noop,
		},
	})
}

const labels = (groups: ReturnType<typeof menuFor>) =>
	groups.map((group) => group.items.map((item) => item.label))

describe('buildBookMoreMenu', () => {
	describe('what each kind of user is offered', () => {
		it('a friend with no permission has the reading items and the file info, and nothing else', () => {
			for (const format of FORMATS) {
				expect(labels(menuFor('friend', format, 'untouched'))).toEqual([
					[KEY.incognito, KEY.markAsRead],
					[KEY.fileInfo],
				])
				expect(labels(menuFor('friend', format, 'reading'))).toEqual([
					[KEY.fromBeginning, KEY.incognito, KEY.markAsRead, KEY.clearProgress],
					[KEY.fileInfo],
				])
				expect(labels(menuFor('friend', format, 'completed'))).toEqual([
					[KEY.fromBeginning, KEY.incognito, KEY.markAsUnread],
					[KEY.fileInfo],
				])
			}
		})

		it("the owner of a PDF they're reading has all of it, with the removal last", () => {
			const groups = menuFor('owner', 'pdf', 'reading')

			expect(labels(groups)).toEqual([
				[KEY.fromBeginning, KEY.incognito, KEY.markAsRead, KEY.clearProgress],
				[KEY.nativePdf, KEY.email],
				[KEY.fileInfo, EDIT_DETAILS_LABEL, KEY.cover, KEY.analyze],
				[REMOVE_FROM_LIBRARY_LABEL],
			])
			const cover = groups[2]?.items.find((item) => item.label === KEY.cover)
			expect(cover?.subItems?.map((item) => item.label)).toEqual([KEY.choosePage, KEY.upload])
		})

		it("an EPUB's cover is changed by an upload, not by picking a page, and it isn't analyzed", () => {
			const groups = menuFor('owner', 'epub', 'untouched')

			const cover = groups.flatMap((group) => group.items).find((item) => item.label === KEY.cover)
			expect(cover?.subItems?.map((item) => item.label)).toEqual([KEY.upload])
			expect(labels(groups).flat()).not.toContain(KEY.analyze)

			// with no upload to offer there is no way to change the cover of an EPUB: no empty item
			expect(labels(menuFor('manager', 'epub', 'untouched')).flat()).not.toContain(KEY.cover)
		})

		it('the removal of a book is a destructive item of its own at the end', () => {
			const groups = menuFor('remover', 'comic', 'untouched')

			expect(labels(groups)).toEqual([
				[KEY.incognito, KEY.markAsRead],
				[KEY.fileInfo],
				[REMOVE_FROM_LIBRARY_LABEL],
			])
			expect(groups.at(-1)?.items[0]?.variant).toBe('destructive')
		})

		it('marking a finished book as unread is destructive (it asks first)', () => {
			const item = menuFor('friend', 'comic', 'completed')[0]?.items.find(
				(item) => item.label === KEY.markAsUnread,
			)

			expect(item?.variant).toBe('destructive')
		})

		it('disables the analysis while one is running', () => {
			const analyze = menuFor('manager', 'comic', 'untouched', { isAnalyzing: true })
				.flatMap((group) => group.items)
				.find((item) => item.label === KEY.analyze)

			expect(analyze?.disabled).toBe(true)
		})
	})

	describe('every kind of user, format and state', () => {
		const combinations = (Object.keys(PEOPLE) as PersonKey[]).flatMap((person) =>
			FORMATS.flatMap((format) =>
				(Object.keys(STATES) as (keyof typeof STATES)[]).map(
					(state) => [person, format, state] as const,
				),
			),
		)

		it('covers six kinds of user, three formats and three states', () => {
			expect(combinations).toHaveLength(6 * 3 * 3)
		})

		it.each(combinations)('%s, %s, %s', (personKey, format, state) => {
			const person = PEOPLE[personKey]
			const has = (permission: UserPermission) => person.permissions.includes(permission)
			const groups = menuFor(personKey, format, state)
			const all = groups.flatMap((group) => group.items)
			const names = all.map((item) => item.label)
			const cover = all.find((item) => item.label === KEY.cover)

			// never an empty group, and never the same item twice
			expect(groups.every((group) => group.items.length > 0)).toBe(true)
			expect(new Set(names).size).toBe(names.length)

			// reading: needs no permission
			expect(names.includes(KEY.incognito)).toBe(true)
			expect(names.includes(KEY.fromBeginning)).toBe(state !== 'untouched')
			expect(names.includes(KEY.markAsRead)).toBe(state !== 'completed')
			expect(names.includes(KEY.clearProgress)).toBe(state === 'reading')
			expect(names.includes(KEY.markAsUnread)).toBe(state === 'completed')
			expect(names.includes(KEY.fileInfo)).toBe(true)

			// what takes a permission is there only for those who have it
			expect(names.includes(KEY.nativePdf)).toBe(
				format === 'pdf' && has(UserPermission.DownloadFile),
			)
			expect(names.includes(KEY.email)).toBe(
				has(UserPermission.EmailSend) || has(UserPermission.EmailArbitrarySend),
			)
			expect(names.includes(EDIT_DETAILS_LABEL)).toBe(has(UserPermission.EditMetadata))
			expect(names.includes(KEY.analyze)).toBe(
				format !== 'epub' && has(UserPermission.ManageLibrary),
			)
			expect(names.includes(REMOVE_FROM_LIBRARY_LABEL)).toBe(has(UserPermission.RemoveBook))
			expect(cover !== undefined).toBe(
				has(UserPermission.EditThumbnails) && (format !== 'epub' || !!person.canUploadCover),
			)
			if (cover) {
				expect(cover.subItems?.map((item) => item.label)).toEqual([
					...(format !== 'epub' ? [KEY.choosePage] : []),
					...(person.canUploadCover ? [KEY.upload] : []),
				])
			}

			// the removal comes last; the download is a button under the cover, not in this menu
			if (names.includes(REMOVE_FROM_LIBRARY_LABEL)) {
				expect(names.at(-1)).toBe(REMOVE_FROM_LIBRARY_LABEL)
			}
			expect(names.some((name) => /download/i.test(name))).toBe(false)
		})
	})
})

describe('pickBookPermissions', () => {
	const pick = (...granted: UserPermission[]) =>
		pickBookPermissions((permission) => granted.includes(permission))

	it('gives a user with no permission none of them', () => {
		expect(pick()).toEqual({
			canDownload: false,
			canEmail: false,
			canEditMetadata: false,
			canEditThumbnail: false,
			canAnalyze: false,
			canRemove: false,
			canManage: false,
		})
	})

	it('shows the way to the manage page only to those it has something for', () => {
		// its tags and metadata take EDIT_METADATA, its metadata search the permission to manage the
		// records; a library manager or a cover editor would arrive at an empty page
		expect(pick(UserPermission.EditMetadata).canManage).toBe(true)
		expect(pick(UserPermission.MetadataFetchRecordManage).canManage).toBe(true)
		expect(pick(UserPermission.ManageLibrary).canManage).toBe(false)
		expect(pick(UserPermission.EditThumbnails).canManage).toBe(false)
	})

	it('lets either email permission send a book', () => {
		expect(pick(UserPermission.EmailSend).canEmail).toBe(true)
		expect(pick(UserPermission.EmailArbitrarySend).canEmail).toBe(true)
	})
})
