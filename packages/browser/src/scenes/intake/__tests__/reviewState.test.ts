import {
	IntakeSeriesKind,
	IntakeSource,
	IntakeState,
	IntakeUploadMode,
	LibraryType,
} from '@stump/graphql'
import { describe, expect, it } from 'vitest'

import type { IntakeItemData } from '../queries'
import {
	acceptableIds,
	ageRatingHides,
	approvableInBulk,
	chosenTags,
	confidenceLevel,
	destinationPaths,
	effectiveGenres,
	fieldSource,
	formatBytes,
	genreChips,
	genresDiffer,
	groupInbox,
	isIntakeFile,
	isSentBack,
	needsAttention,
	needsReviewFirst,
	parseAuthors,
	parseNumber,
	planUploads,
	POLL_FAST_MS,
	POLL_SLOW_MS,
	pollInterval,
	shouldPoll,
	sourceLabel,
	tagKind,
	tagWhy,
	toggleGenre,
	typedSeriesKind,
	type UploadLimits,
	uploadPercent,
	uploadProblem,
	weakGuesses,
} from '../reviewState'
import { item } from './fixtures'

describe('intake review state', () => {
	it('labels sources and finds a field source', () => {
		expect(sourceLabel(IntakeSource.OpenLibrary)).toBe('Open Library')
		expect(sourceLabel(IntakeSource.Hardcover)).toBe('Hardcover')
		expect(sourceLabel(IntakeSource.User)).toBe('You')
		expect(sourceLabel(undefined)).toBeUndefined()
		expect(fieldSource(item().decision?.metadata, 'title')).toBe(IntakeSource.File)
		expect(fieldSource(item().decision?.metadata, 'authors')).toBeUndefined()
	})

	it('grades confidence', () => {
		expect(confidenceLevel(0.9)).toBe('high')
		expect(confidenceLevel(0.5)).toBe('medium')
		expect(confidenceLevel(0.2)).toBe('low')
	})

	it('flags what needs attention', () => {
		expect(needsAttention(item())).toBe(false)
		expect(needsAttention(item({ state: IntakeState.NeedsReview }))).toBe(true)
		expect(needsAttention(item({ state: IntakeState.Failed }))).toBe(true)
		expect(needsAttention(item({ problems: ['Add an author'] }))).toBe(true)
		const withDuplicate = item()
		withDuplicate.proposal?.duplicates.push({
			fileName: 'rosa.epub',
			mediaId: 'm',
			path: '/x',
			reason: 'SAME_TITLE',
			title: 'El nombre de la rosa',
		})
		expect(needsAttention(withDuplicate)).toBe(true)
		// filed and analysing items are never "to review"
		expect(needsAttention(item({ problems: ['x'], state: IntakeState.Filed }))).toBe(false)
	})

	it('groups the inbox and picks what can be accepted in bulk', () => {
		const items = [
			item({ id: '1' }),
			item({ id: '2', state: IntakeState.NeedsReview }),
			item({ id: '3', state: IntakeState.Analysing }),
			item({ id: '4', state: IntakeState.Filed }),
			item({ id: '5', state: IntakeState.Discarded }),
			item({ id: '6', problems: ['Choose a folder'] }),
		]
		const groups = groupInbox(items)
		expect(groups.ready.map((i) => i.id)).toEqual(['1'])
		expect(groups.toReview.map((i) => i.id)).toEqual(['2', '6'])
		expect(groups.analysing.map((i) => i.id)).toEqual(['3'])
		expect(groups.filed.map((i) => i.id)).toEqual(['4'])
		expect(acceptableIds(items)).toEqual(['1'])
	})

	it('polls while analysing or waiting for a recent scan', () => {
		expect(shouldPoll([item()])).toBe(false)
		expect(shouldPoll([item({ state: IntakeState.Analysing })])).toBe(true)
		const justFiled = item({ filedAt: new Date().toISOString(), state: IntakeState.Filed })
		expect(shouldPoll([justFiled])).toBe(true)
		expect(shouldPoll([{ ...justFiled, mediaIds: ['m'] }])).toBe(false)
		const longAgo = item({ filedAt: '2020-01-01T00:00:00Z', state: IntakeState.Filed })
		expect(shouldPoll([longAgo])).toBe(false)
	})

	it('edits tags and parses fields', () => {
		expect(toggleGenre(['A', 'B'], 'A')).toEqual(['B'])
		expect(toggleGenre(['A'], 'B')).toEqual(['A', 'B'])
		expect(parseAuthors('Gabriel García Márquez; Mercedes Barcha ;')).toEqual([
			'Gabriel García Márquez',
			'Mercedes Barcha',
		])
		expect(parseNumber('12,5')).toBe(12.5)
		expect(parseNumber('')).toBeUndefined()
		expect(parseNumber('abc')).toBeUndefined()
		expect(tagKind('Origin: Italian')).toBe('FACET')
		expect(tagKind('Illustrated')).toBe('FACET')
		expect(tagKind('Science Fiction: Space Opera')).toBe('GENRE')
	})

	it('lists chips: suggestions first, then tags the uploader added', () => {
		const withExtra = item()
		withExtra.decision?.metadata.genres.push('Mystery')
		const chips = genreChips(withExtra)
		expect(chips.map((c) => [c.value, c.chosen, c.suggested])).toEqual([
			['Historical Fiction', true, true],
			['Origin: Italian', false, true],
			['Mystery', true, false],
		])
		expect(destinationPaths(withExtra)).toEqual(['Umberto_Eco/El_nombre_de_la_rosa.epub'])
	})

	it('tells tags backed by the book apart from guesses of similar books', () => {
		const chips = genreChips(item())
		expect(chips.map((c) => [c.value, c.origin, c.preselected])).toEqual([
			['Historical Fiction', 'guessed', true],
			['Origin: Italian', 'guessed', false],
		])

		const withSubject = item()
		withSubject.proposal?.genreSuggestions.push(
			{
				confidence: 0.6,
				kind: 'GENRE',
				libraryCount: 3,
				reasons: ["subject 'Space opera'", '1 of 4 books by this author have it'],
				selected: true,
				value: 'Science Fiction: Space Opera',
			},
			{
				confidence: 0.6,
				kind: 'GENRE',
				libraryCount: 9,
				reasons: ['the parent of a suggested tag'],
				selected: true,
				value: 'Science Fiction',
			},
			{
				confidence: 0.95,
				kind: 'FACET',
				libraryCount: 9,
				reasons: ['first published in 1980'],
				selected: true,
				value: 'Era: 20th Century',
			},
		)
		const origins = Object.fromEntries(genreChips(withSubject).map((c) => [c.value, c.origin]))
		expect(origins['Science Fiction: Space Opera']).toBe('book')
		// a parent is as good as its children
		expect(origins['Science Fiction']).toBe('book')
		expect(origins['Era: 20th Century']).toBe('book')

		const guessedChild = item()
		guessedChild.proposal?.genreSuggestions.push(
			{
				confidence: 0.6,
				kind: 'GENRE',
				libraryCount: 3,
				reasons: ['2 of 4 books by this author have it'],
				selected: true,
				value: 'Science Fiction: Space Opera',
			},
			{
				confidence: 0.6,
				kind: 'GENRE',
				libraryCount: 9,
				reasons: ['the parent of a suggested tag'],
				selected: true,
				value: 'Science Fiction',
			},
		)
		const guessedOrigins = Object.fromEntries(
			genreChips(guessedChild).map((c) => [c.value, c.origin]),
		)
		expect(guessedOrigins['Science Fiction']).toBe('guessed')
	})

	it('leaves weak guesses unticked until the uploader chooses the tags', () => {
		// Historical Fiction is pre-selected only from author votes at 0.8 (< 0.85)
		expect(weakGuesses(item())).toEqual(['Historical Fiction'])
		expect(effectiveGenres(item(), false)).toEqual([])
		expect(effectiveGenres(item(), true)).toEqual(['Historical Fiction'])
		const strong = item()
		const first = strong.proposal?.genreSuggestions[0]
		if (first) first.confidence = 0.9
		expect(weakGuesses(strong)).toEqual([])
		expect(effectiveGenres(strong, false)).toEqual(['Historical Fiction'])
		// filing sends the tags when they differ from the server's
		expect(genresDiffer(item(), [])).toBe(true)
		expect(genresDiffer(item(), ['Historical Fiction'])).toBe(false)
	})

	it('explains each tag in one line', () => {
		const [historical] = genreChips(item())
		expect(historical && tagWhy(historical)).toBe('guessed: 3 of 4 books by this author have it')
		expect(tagWhy({ origin: 'book', reasons: ["subject 'Rome'", 'first published in 1980'] })).toBe(
			"subject 'Rome' (+1 more)",
		)
		expect(tagWhy({ origin: 'you', reasons: ['Added by you'] })).toBe('added by you')
		expect(
			tagWhy({
				origin: 'book',
				reasons: ['8 of 8 books by this author have it', 'manga / light novel'],
			}),
		).toBe('manga / light novel (+1 more)')
		const withExtra = item()
		withExtra.decision?.metadata.genres.push('Mystery')
		expect(chosenTags(withExtra)).toEqual([
			{ origin: 'guessed', value: 'Historical Fiction' },
			{ origin: 'you', value: 'Mystery' },
		])
		expect(chosenTags(withExtra, ['Mystery'])).toEqual([{ origin: 'you', value: 'Mystery' }])
	})

	it('plans uploads: one request per loose book, one per folder, books only', () => {
		const file = (name: string, size = 10) => new File(['x'.repeat(size)], name)
		const batches = planUploads([
			{ file: file('rosa.epub') },
			{ file: file('notes.txt') },
			{ file: file('Yotsuba v15.cbz', 5), relativePath: 'Yotsuba/Yotsuba v15.cbz' },
			{ file: file('Yotsuba v16.cbz', 7), relativePath: 'Yotsuba/Yotsuba v16.cbz' },
			{ file: file('.DS_Store'), relativePath: 'Yotsuba/.DS_Store' },
			{ file: file('cover.jpg'), relativePath: 'Yotsuba/cover.jpg' },
			{ file: file('pack.zip') },
		])
		expect(batches.map((b) => [b.label, b.files.length, b.isFolder])).toEqual([
			['rosa.epub', 1, false],
			['Yotsuba', 2, true],
			['pack.zip', 1, false],
		])
		expect(batches[1]?.relativePaths).toEqual([
			'Yotsuba/Yotsuba v15.cbz',
			'Yotsuba/Yotsuba v16.cbz',
		])
		expect(batches[1]?.totalBytes).toBe(12)
		expect(batches[1]?.mode).toBe(IntakeUploadMode.Auto)
		expect(isIntakeFile('Book.PDF')).toBe(true)
		expect(isIntakeFile('a/b/.hidden.epub')).toBe(false)
	})

	it('computes upload progress', () => {
		expect(uploadPercent(50, 200)).toBe(25)
		expect(uploadPercent(10)).toBe(0)
		expect(uploadPercent(300, 200)).toBe(100)
	})
})

describe('typedSeriesKind', () => {
	it('is the kind of series a manga or light novel library holds', () => {
		expect(typedSeriesKind(LibraryType.Manga)).toBe(IntakeSeriesKind.Manga)
		expect(typedSeriesKind(LibraryType.LightNovel)).toBe(IntakeSeriesKind.LightNovel)
	})

	it('is null for the libraries that file books under their author', () => {
		for (const type of [
			LibraryType.Book,
			LibraryType.Mixed,
			LibraryType.Comic,
			LibraryType.Manhwa,
			LibraryType.WebNovel,
			LibraryType.Webtoon,
		]) {
			expect(typedSeriesKind(type)).toBeNull()
		}
		expect(typedSeriesKind(undefined)).toBeNull()
		expect(typedSeriesKind(null)).toBeNull()
	})
})

describe('the inbox of somebody who needs approval', () => {
	const sentBack = (overrides = {}) =>
		item({ reviewNote: 'The author is spelled wrong', state: IntakeState.Rejected, ...overrides })

	it('tells a book an approver sent back, changed or not', () => {
		expect(isSentBack(sentBack())).toBe(true)
		// a change makes it ready or in need of a look again, the note stays until it is sent anew
		expect(isSentBack(item({ reviewNote: 'Fix it', state: IntakeState.Ready }))).toBe(true)
		expect(isSentBack(item({ reviewNote: 'Fix it', state: IntakeState.NeedsReview }))).toBe(true)
		expect(isSentBack(item())).toBe(false)
		expect(isSentBack(item({ reviewNote: null }))).toBe(false)
		// a book that is on its way to the library, or in it, is not "sent back" any more
		expect(isSentBack(item({ reviewNote: 'Old note', state: IntakeState.PendingApproval }))).toBe(
			false,
		)
		expect(isSentBack(item({ reviewNote: 'Old note', state: IntakeState.Filed }))).toBe(false)
	})

	it('groups what waits for an approver, what is being filed and what was sent back', () => {
		const groups = groupInbox([
			item({ id: 'ready' }),
			item({ id: 'waiting', state: IntakeState.PendingApproval }),
			item({ id: 'filing', state: IntakeState.Filing }),
			sentBack({ id: 'back' }),
			item({ id: 'edited', reviewNote: 'Fix it', state: IntakeState.Ready }),
			item({ id: 'filed', state: IntakeState.Filed }),
			item({ id: 'check', state: IntakeState.NeedsReview }),
		])

		expect(groups.ready.map((i) => i.id)).toEqual(['ready'])
		expect(groups.waiting.map((i) => i.id)).toEqual(['waiting'])
		expect(groups.filing.map((i) => i.id)).toEqual(['filing'])
		expect(groups.sentBack.map((i) => i.id)).toEqual(['back', 'edited'])
		expect(groups.filed.map((i) => i.id)).toEqual(['filed'])
		expect(groups.toReview.map((i) => i.id)).toEqual(['check'])
	})

	it('does not offer a sent back book in "send all ready"', () => {
		// it goes again one by one, after what the note asks for: changed or not
		const edited = item({ id: 'c', reviewNote: 'Fix it', state: IntakeState.Ready })

		expect(acceptableIds([item({ id: 'a' }), sentBack({ id: 'b' }), edited])).toEqual(['a'])
	})

	it('polls fast while a book is filed after its approval, slowly while it waits for one', () => {
		expect(pollInterval([item()])).toBe(false)
		expect(pollInterval([item({ state: IntakeState.Analysing })])).toBe(POLL_FAST_MS)
		expect(pollInterval([item({ state: IntakeState.Filing })])).toBe(POLL_FAST_MS)
		expect(pollInterval([item({ state: IntakeState.PendingApproval })])).toBe(POLL_SLOW_MS)
		// what moves wins over what waits
		expect(
			pollInterval([
				item({ state: IntakeState.PendingApproval }),
				item({ state: IntakeState.Analysing }),
			]),
		).toBe(POLL_FAST_MS)
		expect(shouldPoll([item({ state: IntakeState.Filing })])).toBe(true)
	})
})

describe('the upload pre-check', () => {
	const MB = 1024 * 1024
	const limits: UploadLimits = {
		maxFileBytes: 100 * MB,
		maxFilesPerRequest: 5,
		maxItems: 10,
		maxRequestBytes: 300 * MB,
		openItems: 2,
		quotaApplies: true,
		quotaBytes: 1000 * MB,
		uploadsOpen: true,
		usedBytes: 100 * MB,
	}
	const file = (name: string, mb: number) => ({ name, size: mb * MB }) as File
	const batch = (files: File[], isFolder = false) => ({
		files,
		isFolder,
		label: files[0]?.name ?? '',
		totalBytes: files.reduce((sum, f) => sum + f.size, 0),
	})

	it('lets an upload that fits go', () => {
		expect(uploadProblem(batch([file('a.epub', 10)]), limits)).toBeUndefined()
	})

	it("says why uploads are closed, in the server's words when it gave any", () => {
		expect(
			uploadProblem(batch([file('a.epub', 1)]), {
				...limits,
				closedReason: 'The owner has turned uploads off',
				uploadsOpen: false,
			}),
		).toBe('The owner has turned uploads off')
		expect(uploadProblem(batch([file('a.epub', 1)]), { ...limits, uploadsOpen: false })).toBe(
			'Uploads are closed right now',
		)
	})

	it('refuses a file over the biggest a book may be, naming it', () => {
		const problem = uploadProblem(batch([file('small.epub', 1), file('huge.cbz', 150)]), limits)

		expect(problem).toContain('huge.cbz')
		expect(problem).toContain('150.0 MB')
		expect(problem).toContain('100.0 MB')
	})

	it('refuses too many files in one go, for a folder and for loose files', () => {
		const many = Array.from({ length: 6 }, (_, i) => file(`v${i}.cbz`, 1))

		expect(uploadProblem(batch(many, true), limits)).toBe('A folder can hold at most 5 files')
		expect(uploadProblem(batch(many), limits)).toBe('An upload can hold at most 5 files')
	})

	it('refuses more than one request can hold, though every file is small enough', () => {
		const files = Array.from({ length: 4 }, (_, i) => file(`v${i}.cbz`, 90))

		expect(uploadProblem(batch(files, true), limits)).toContain('smaller parts')
	})

	it('refuses what does not fit in the room left in the inbox', () => {
		const problem = uploadProblem(batch([file('a.epub', 60)]), { ...limits, usedBytes: 980 * MB })

		expect(problem).toContain('20.0 MB free')
		expect(problem).toContain('60.0 MB')
	})

	it('refuses a book when the inbox already holds as many as it may', () => {
		expect(uploadProblem(batch([file('a.epub', 1)]), { ...limits, openItems: 10 })).toBe(
			'Your inbox is full (10 books): send or discard some first',
		)
	})

	it('does not hold the quota against somebody it does not apply to', () => {
		const unlimited = { ...limits, openItems: 10, quotaApplies: false, usedBytes: 5000 * MB }

		expect(uploadProblem(batch([file('a.epub', 60)]), unlimited)).toBeUndefined()
	})
})

describe('an age rating and the person who sent the book', () => {
	it('hides a book rated above the limit', () => {
		expect(ageRatingHides(18, { age: 13, restrictOnUnset: false })).toBe('hidden')
		expect(ageRatingHides(13, { age: 13, restrictOnUnset: false })).toBeNull()
		expect(ageRatingHides(0, { age: 13, restrictOnUnset: true })).toBeNull()
	})

	it('hides a book with no rating only from somebody who reads rated books only', () => {
		expect(ageRatingHides(null, { age: 13, restrictOnUnset: true })).toBe('unrated')
		expect(ageRatingHides(undefined, { age: 13, restrictOnUnset: true })).toBe('unrated')
		expect(ageRatingHides(null, { age: 13, restrictOnUnset: false })).toBeNull()
	})

	it('has nothing to say when the person has no limit, or it is not known', () => {
		expect(ageRatingHides(18, null)).toBeNull()
		expect(ageRatingHides(18, undefined)).toBeNull()
	})
})

describe('sizes', () => {
	it('writes bytes in the unit that reads best', () => {
		expect(formatBytes(500)).toBe('1 KB')
		expect(formatBytes(2048)).toBe('2 KB')
		expect(formatBytes(5 * 1024 * 1024)).toBe('5.0 MB')
		expect(formatBytes(3 * 1024 * 1024 * 1024)).toBe('3.0 GB')
	})
})

describe('a review that has to come first', () => {
	const tagged = (state: IntakeState, genres: string[]) => {
		const base = item({ state })
		if (!base.decision) throw new Error('the fixture has a decision')
		return {
			...base,
			decision: { ...base.decision, metadata: { ...base.decision.metadata, genres } },
		}
	}

	it('is not needed for a book that is ready, with the tags the server has', () => {
		expect(needsReviewFirst(tagged(IntakeState.Ready, ['Fantasy']), ['Fantasy'])).toBe(false)
		expect(
			needsReviewFirst(tagged(IntakeState.PendingApproval, ['Fantasy', 'Horror']), [
				'Horror',
				'Fantasy',
			]),
		).toBe(false)
	})

	it('is needed when the tags written are not the server’s', () => {
		expect(needsReviewFirst(tagged(IntakeState.Ready, ['Fantasy', 'Horror']), ['Fantasy'])).toBe(
			true,
		)
		expect(needsReviewFirst(tagged(IntakeState.PendingApproval, []), ['Fantasy'])).toBe(true)
	})

	it('is needed for a book flagged for a look, and for one an approver sent back', () => {
		expect(needsReviewFirst(tagged(IntakeState.NeedsReview, ['Fantasy']), ['Fantasy'])).toBe(true)
		// the only way to open it for sending again, whether or not the person changed anything
		expect(needsReviewFirst(tagged(IntakeState.Rejected, ['Fantasy']), ['Fantasy'])).toBe(true)
	})
})

describe('what "Approve all" takes', () => {
	const waiting = (overrides: Partial<IntakeItemData> = {}) =>
		item({ state: IntakeState.PendingApproval, ...overrides })
	const duplicate = {
		fileName: 'rosa.epub',
		mediaId: 'm1',
		path: '',
		reason: 'SAME_TITLE',
		title: 'El nombre de la rosa',
	}

	it('takes a book with nothing in its way', () => {
		expect(approvableInBulk(waiting())).toBe(true)
	})

	it('leaves out a book without a decision, or with something left to fix', () => {
		expect(approvableInBulk(waiting({ decision: null }))).toBe(false)
		expect(approvableInBulk(waiting({ problems: ['Choose a folder'] }))).toBe(false)
	})

	it('leaves out a book that may already be in the library: it is looked at on its own', () => {
		const base = item()
		expect(
			approvableInBulk(
				waiting({ proposal: base.proposal ? { ...base.proposal, duplicates: [duplicate] } : null }),
			),
		).toBe(false)
	})

	it('leaves out a book that failed before', () => {
		expect(approvableInBulk(waiting({ error: 'Could not be written' }))).toBe(false)
	})
})
