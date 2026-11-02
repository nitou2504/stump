import { IntakeKind, IntakeSource, IntakeState, IntakeUploadMode } from '@stump/graphql'
import { describe, expect, it } from 'vitest'

import type { IntakeItemData } from '../queries'
import {
	acceptableIds,
	chosenTags,
	confidenceLevel,
	destinationPaths,
	effectiveGenres,
	fieldSource,
	genreChips,
	genresDiffer,
	groupInbox,
	isIntakeFile,
	needsAttention,
	parseAuthors,
	parseNumber,
	planUploads,
	shouldPoll,
	sourceLabel,
	tagKind,
	tagWhy,
	toggleGenre,
	uploadPercent,
	weakGuesses,
} from '../reviewState'

const item = (overrides: Partial<IntakeItemData> = {}): IntakeItemData => ({
	createdAt: '2026-11-01T00:00:00Z',
	decision: {
		fileNames: ['El_nombre_de_la_rosa.epub'],
		folder: 'Umberto_Eco',
		metadata: {
			authors: ['Umberto Eco'],
			genres: ['Historical Fiction'],
			sources: [{ field: 'title', source: IntakeSource.File }],
			title: 'El nombre de la rosa',
		},
		volumes: [null],
	},
	destination: {
		dirExists: true,
		files: [
			{
				exists: false,
				originalName: 'rosa.epub',
				relativePath: 'Umberto_Eco/El_nombre_de_la_rosa.epub',
			},
		],
		relativeDir: 'Umberto_Eco',
	},
	files: [{ format: 'epub', originalName: 'rosa.epub', size: 1000 }],
	id: 'a',
	kind: IntakeKind.Book,
	libraryId: 'lib',
	libraryName: 'Biblioteca',
	mediaIds: [],
	name: 'rosa.epub',
	problems: [],
	proposal: {
		authorFolders: [],
		confidence: 0.9,
		duplicates: [],
		genreSuggestions: [
			{
				confidence: 0.8,
				kind: 'GENRE',
				libraryCount: 40,
				reasons: ['3 of 4 books by this author have it'],
				selected: true,
				value: 'Historical Fiction',
			},
			{
				confidence: 0.3,
				kind: 'FACET',
				libraryCount: 12,
				reasons: ['2 books with a similar description have it'],
				selected: false,
				value: 'Origin: Italian',
			},
		],
		lookedUpOnline: true,
		metadata: { authors: ['Umberto Eco'], genres: [], sources: [], subjects: [] },
		seriesFolders: [],
		similarBooks: [],
		warnings: [],
	},
	state: IntakeState.Ready,
	...overrides,
})

describe('intake review state', () => {
	it('labels sources and finds a field source', () => {
		expect(sourceLabel(IntakeSource.OpenLibrary)).toBe('Open Library')
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
