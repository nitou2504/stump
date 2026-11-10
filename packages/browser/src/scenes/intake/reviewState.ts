import {
	IntakeSeriesKind,
	IntakeSource,
	IntakeState,
	IntakeUploadMode,
	LibraryType,
} from '@stump/graphql'

import type { IntakeItemData } from './queries'

/** The book formats the intake accepts (plus .zip packs of books) */
export const INTAKE_EXTENSIONS = ['epub', 'pdf', 'cbz', 'cbr', 'zip'] as const

const SOURCE_LABELS: Record<IntakeSource, string> = {
	[IntakeSource.File]: 'File',
	[IntakeSource.FileName]: 'File name',
	[IntakeSource.OpenLibrary]: 'Open Library',
	[IntakeSource.GoogleBooks]: 'Google Books',
	[IntakeSource.Hardcover]: 'Hardcover',
	[IntakeSource.Library]: 'Library',
	[IntakeSource.User]: 'You',
}

export const sourceLabel = (source?: IntakeSource | null) =>
	source ? SOURCE_LABELS[source] : undefined

type WithSources = { sources: Array<{ field: string; source: IntakeSource }> }

/** Where a metadata field's value came from, e.g. `Open Library` */
export function fieldSource(metadata: WithSources | null | undefined, field: string) {
	return metadata?.sources.find((s) => s.field === field)?.source ?? undefined
}

export const SERIES_KIND_LABELS: Record<IntakeSeriesKind, string> = {
	[IntakeSeriesKind.Manga]: 'Manga',
	[IntakeSeriesKind.LightNovel]: 'Light novel',
}

/**
 * The kind of series a manga or light novel library holds (one folder per series at its root,
 * nothing else), or `null` for any other library, which files books under their author
 */
export function typedSeriesKind(libraryType?: LibraryType | null): IntakeSeriesKind | null {
	switch (libraryType) {
		case LibraryType.Manga:
			return IntakeSeriesKind.Manga
		case LibraryType.LightNovel:
			return IntakeSeriesKind.LightNovel
		default:
			return null
	}
}

export type ConfidenceLevel = 'high' | 'medium' | 'low'

export function confidenceLevel(confidence: number): ConfidenceLevel {
	if (confidence >= 0.75) return 'high'
	if (confidence >= 0.45) return 'medium'
	return 'low'
}

/** Items that can be filed as they are */
export function isReady(item: Pick<IntakeItemData, 'state' | 'problems'>) {
	return item.state === IntakeState.Ready && item.problems.length === 0
}

/**
 * Whether "Approve all" takes a book that waits for approval: it has a decision, nothing blocks
 * it, nothing says it may already be in the library and it did not fail before. The others are
 * for the approver to look at on their own card: a copy of a book somebody may not read can be
 * among them, and nobody should file it in one click.
 */
export function approvableInBulk(
	item: Pick<IntakeItemData, 'decision' | 'problems' | 'proposal' | 'error'>,
) {
	return (
		!!item.decision &&
		item.problems.length === 0 &&
		(item.proposal?.duplicates.length ?? 0) === 0 &&
		!item.error
	)
}

/** Items that want the uploader's attention before filing */
export function needsAttention(
	item: Pick<IntakeItemData, 'state' | 'problems' | 'proposal'>,
): boolean {
	if (item.state === IntakeState.NeedsReview || item.state === IntakeState.Failed) return true
	if (item.state === IntakeState.Filed || item.state === IntakeState.Analysing) return false
	return item.problems.length > 0 || (item.proposal?.duplicates.length ?? 0) > 0
}

export type InboxGroups = {
	analysing: IntakeItemData[]
	toReview: IntakeItemData[]
	ready: IntakeItemData[]
	/** Approved and being written into the library */
	filing: IntakeItemData[]
	/** Sent for approval, waiting for an approver */
	waiting: IntakeItemData[]
	/** An approver sent them back with a note: the uploader changes them and sends them again */
	sentBack: IntakeItemData[]
	filed: IntakeItemData[]
}

/**
 * An item an approver sent back: in the state they left it in, or changed since (a change makes
 * it ready or in need of a look again, and the note stays until it is sent anew)
 */
export function isSentBack(item: Pick<IntakeItemData, 'state' | 'reviewNote'>): boolean {
	if (item.state === IntakeState.Rejected) return true
	return (
		!!item.reviewNote &&
		(item.state === IntakeState.Ready || item.state === IntakeState.NeedsReview)
	)
}

/**
 * Splits the inbox into what the page shows: items being analysed, items to review (needs
 * review, failed, or with problems/duplicates), items ready to file or send, items an approver
 * sent back, items waiting for approval or being filed after it, and items already filed. The
 * server returns them newest first; the order is kept.
 */
export function groupInbox(items: IntakeItemData[]): InboxGroups {
	const groups: InboxGroups = {
		analysing: [],
		filed: [],
		filing: [],
		ready: [],
		sentBack: [],
		toReview: [],
		waiting: [],
	}
	for (const item of items) {
		if (item.state === IntakeState.Discarded) continue
		if (item.state === IntakeState.Analysing) groups.analysing.push(item)
		else if (item.state === IntakeState.Filed) groups.filed.push(item)
		else if (item.state === IntakeState.Filing) groups.filing.push(item)
		else if (item.state === IntakeState.PendingApproval) groups.waiting.push(item)
		else if (isSentBack(item)) groups.sentBack.push(item)
		else if (needsAttention(item)) groups.toReview.push(item)
		else groups.ready.push(item)
	}
	return groups
}

/**
 * The ids "Add all ready" (or "Send all ready") takes: ready, nothing blocking, no duplicate
 * warning, and not a book an approver sent back (that one goes again by itself, once what the
 * note asks for is done)
 */
export function acceptableIds(items: IntakeItemData[]) {
	return items
		.filter((item) => isReady(item) && !needsAttention(item) && !isSentBack(item))
		.map(({ id }) => id)
}

/** How often the inbox is read while something in it is moving (ms) */
export const POLL_FAST_MS = 1500
/** How often it is read while books wait for an approver, who may take hours */
export const POLL_SLOW_MS = 30_000

/**
 * Whether the inbox should be polled fast: something is being analysed, being filed after an
 * approval, or was filed and not scanned yet
 */
export function shouldPoll(items: IntakeItemData[]) {
	return items.some(
		(item) =>
			item.state === IntakeState.Analysing ||
			item.state === IntakeState.Filing ||
			(item.state === IntakeState.Filed &&
				item.mediaIds.length === 0 &&
				!!item.filedAt &&
				Date.now() - new Date(item.filedAt).getTime() < 5 * 60 * 1000),
	)
}

/**
 * How long to wait before reading the inbox again, or `false` for never: fast while something
 * moves, slowly while books wait for approval (so an answer shows without a reload)
 */
export function pollInterval(items: IntakeItemData[]): number | false {
	if (shouldPoll(items)) return POLL_FAST_MS
	if (items.some((item) => item.state === IntakeState.PendingApproval)) return POLL_SLOW_MS
	return false
}

/** Adds or removes a tag, keeping the order and without duplicates */
export function toggleGenre(genres: string[], value: string): string[] {
	return genres.includes(value) ? genres.filter((g) => g !== value) : [...genres, value]
}

/** `Gabriel García Márquez; Mercedes Barcha` → two authors (`;` or a line break separate) */
export function parseAuthors(value: string): string[] {
	return value
		.split(/[;\n]/)
		.map((a) => a.trim())
		.filter(Boolean)
}

export const formatAuthors = (authors: string[]) => authors.join('; ')

/** `12`, `12.5` → a number; empty or invalid → undefined */
export function parseNumber(value: string): number | undefined {
	const trimmed = value.trim().replace(',', '.')
	if (!trimmed) return undefined
	const number = Number(trimmed)
	return Number.isFinite(number) ? number : undefined
}

/** The full destination of each file, e.g. `Umberto_Eco/El_nombre_de_la_rosa.epub` */
export function destinationPaths(item: Pick<IntakeItemData, 'destination'>): string[] {
	return item.destination?.files.map((file) => file.relativePath) ?? []
}

/** The facet prefixes of the library's tag scheme (the rest are genres) */
const FACET_PREFIXES = [
	'Origin:',
	'Era:',
	'Level:',
	'Audience:',
	'Form:',
	'Canon:',
	'Award:',
	'Laureate:',
	'Source:',
	'Course:',
	'Collection:',
]

export const tagKind = (value: string) =>
	value === 'Illustrated' || FACET_PREFIXES.some((prefix) => value.startsWith(prefix))
		? 'FACET'
		: 'GENRE'

export type TagOrigin = 'book' | 'guessed' | 'you'

export type GenreChip = {
	value: string
	confidence: number
	reasons: string[]
	libraryCount: number
	kind: string
	/** Part of what will be written */
	chosen: boolean
	/** Proposed by the server (as opposed to added by the uploader) */
	suggested: boolean
	/** Part of the server's default choice */
	preselected: boolean
	/**
	 * `book`: backed by the book itself (a subject in its file or found online, its year,
	 * manga / light novel); `guessed`: only votes of other books (the author's or the
	 * series' other books, similar descriptions); `you`: added by the uploader
	 */
	origin: TagOrigin
}

/** Guessed tags under this confidence are not ticked until the uploader ticks them */
export const GUESS_TICK_THRESHOLD = 0.85

// "3 of 9 books by this author have it", "4 books with a similar description have it"
const VOTE_REASON = /^\d+ (of \d+ .+|books with a similar description) have it$/
const PARENT_REASON = 'the parent of a suggested tag'

export const isVoteReason = (reason: string) => VOTE_REASON.test(reason)

type ChipSource = Pick<IntakeItemData, 'proposal' | 'decision'>

/**
 * Every suggestion with where it comes from. A `Big` tag proposed only as the parent of
 * suggested `Big: Small` tags is guessed when all those children are guessed.
 */
function suggestionsWithOrigin(item: ChipSource) {
	const suggestions = item.proposal?.genreSuggestions ?? []
	const base = suggestions.map((s) => ({
		...s,
		origin: (s.reasons.length > 0 && s.reasons.every(isVoteReason)
			? 'guessed'
			: 'book') as TagOrigin,
	}))
	return base.map((s) => {
		if (!(s.reasons.length > 0 && s.reasons.every((r) => r === PARENT_REASON || isVoteReason(r)))) {
			return s
		}
		if (!s.reasons.includes(PARENT_REASON)) return s
		const children = base.filter((c) => c.value.startsWith(`${s.value}:`) && c.selected)
		const guessed = children.length > 0 && children.every((c) => c.origin === 'guessed')
		return { ...s, origin: (guessed ? 'guessed' : 'book') as TagOrigin }
	})
}

/** Pre-selected guesses that stay unticked until the uploader ticks them */
export function weakGuesses(item: ChipSource): string[] {
	return suggestionsWithOrigin(item)
		.filter((s) => s.selected && s.origin === 'guessed' && s.confidence < GUESS_TICK_THRESHOLD)
		.map((s) => s.value)
}

/**
 * The tags that will be written. Until the uploader has touched the tags, the server's
 * choice minus its weak guesses (so a wrong author vote isn't written silently); after
 * that, exactly what the uploader chose.
 */
export function effectiveGenres(item: ChipSource, tagsReviewed: boolean): string[] {
	const genres = item.decision?.metadata.genres ?? []
	if (tagsReviewed) return genres
	const weak = new Set(weakGuesses(item))
	return genres.filter((g) => !weak.has(g))
}

/** Whether filing needs the tags sent first (the written tags differ from the server's) */
export function genresDiffer(item: ChipSource, genres: string[]) {
	const current = item.decision?.metadata.genres ?? []
	return current.length !== genres.length || current.some((g) => !genres.includes(g))
}

/**
 * Whether a review has to go to the server before the book is sent, added or approved, even
 * with a review's worth of nothing: the written tags differ from the server's; the book was
 * flagged for a look (a review of one with nothing blocking makes it ready); or an approver sent
 * it back, where a review is the only way to open it for sending again, changed or not.
 */
export function needsReviewFirst(
	item: ChipSource & Pick<IntakeItemData, 'state'>,
	genres: string[],
) {
	return (
		item.state === IntakeState.NeedsReview ||
		item.state === IntakeState.Rejected ||
		genresDiffer(item, genres)
	)
}

/** The tags shown as chips: the suggestions first, then any extra chosen tag */
export function genreChips(
	item: ChipSource,
	chosen = item.decision?.metadata.genres ?? [],
): GenreChip[] {
	const suggestions = suggestionsWithOrigin(item)
	const suggested = new Set(suggestions.map((s) => s.value))
	return [
		...suggestions.map(({ value, confidence, reasons, libraryCount, kind, selected, origin }) => ({
			chosen: chosen.includes(value),
			confidence,
			kind,
			libraryCount,
			origin,
			preselected: selected,
			reasons,
			suggested: true,
			value,
		})),
		...chosen
			.filter((g) => !suggested.has(g))
			.map((value) => ({
				chosen: true,
				confidence: 1,
				kind: tagKind(value),
				libraryCount: 0,
				origin: 'you' as TagOrigin,
				preselected: false,
				reasons: ['Added by you'],
				suggested: false,
				value,
			})),
	]
}

/** The tags that will be written, each with where it comes from */
export function chosenTags(item: ChipSource, chosen?: string[]) {
	return genreChips(item, chosen)
		.filter((chip) => chip.chosen)
		.map((chip) => ({ origin: chip.origin, value: chip.value }))
}

/** One short line explaining a tag, e.g. `subject 'Fantasy fiction'` */
export function tagWhy(chip: Pick<GenreChip, 'reasons' | 'origin'>) {
	if (chip.origin === 'you') return 'added by you'
	// for a tag backed by the book, lead with that evidence rather than the votes
	const reasons =
		chip.origin === 'book'
			? [...chip.reasons.filter((r) => !isVoteReason(r)), ...chip.reasons.filter(isVoteReason)]
			: chip.reasons
	const [first, ...rest] = reasons
	const reason = first ?? 'suggested'
	return `${chip.origin === 'guessed' ? 'guessed: ' : ''}${reason}${rest.length ? ` (+${rest.length} more)` : ''}`
}

export type PendingFile = {
	file: File
	/** `webkitRelativePath` for files picked or dropped as a folder */
	relativePath?: string
}

export type UploadBatch = {
	key: string
	/** The folder name, or the file name for a single book */
	label: string
	files: File[]
	/** Set for folder uploads, same order as `files` */
	relativePaths?: string[]
	isFolder: boolean
	mode: IntakeUploadMode
	totalBytes: number
}

export function extensionOf(name: string) {
	const dot = name.lastIndexOf('.')
	return dot === -1 ? '' : name.slice(dot + 1).toLowerCase()
}

/** Whether a picked file is a book the intake takes (hidden files and extras are skipped) */
export function isIntakeFile(name: string) {
	const base = name.split('/').pop() ?? name
	if (!base || base.startsWith('.')) return false
	return (INTAKE_EXTENSIONS as readonly string[]).includes(extensionOf(base))
}

/**
 * Groups picked files into upload requests: each loose file is its own request (so each
 * gets its own progress and a failure doesn't stop the others), and the files of a picked
 * folder go together so the server can make them one series. Files that aren't books are
 * left out.
 */
export function planUploads(entries: PendingFile[]): UploadBatch[] {
	const batches: UploadBatch[] = []
	const folders = new Map<string, UploadBatch>()

	for (const { file, relativePath } of entries) {
		const path = relativePath?.replace(/^\/+/, '') || ''
		if (!isIntakeFile(path || file.name)) continue

		const folder = path.includes('/') ? path.split('/')[0] : undefined
		if (!folder) {
			batches.push({
				files: [file],
				isFolder: false,
				key: `file:${file.name}:${file.size}:${file.lastModified}`,
				label: file.name,
				mode: IntakeUploadMode.Auto,
				totalBytes: file.size,
			})
			continue
		}

		let batch = folders.get(folder)
		if (!batch) {
			batch = {
				files: [],
				isFolder: true,
				key: `folder:${folder}`,
				label: folder,
				mode: IntakeUploadMode.Auto,
				relativePaths: [],
				totalBytes: 0,
			}
			folders.set(folder, batch)
			batches.push(batch)
		}
		batch.files.push(file)
		batch.relativePaths?.push(path)
		batch.totalBytes += file.size
	}

	return batches
}

/** Percent of a batch uploaded, from the request's loaded/total bytes */
export function uploadPercent(loaded: number, total?: number) {
	if (!total || total <= 0) return 0
	return Math.min(100, Math.round((loaded * 100) / total))
}

/** `1.5 GB`, `12.3 MB`, `800 KB` */
export function formatBytes(bytes: number) {
	return bytes >= 1024 * 1024 * 1024
		? `${(bytes / 1024 / 1024 / 1024).toFixed(1)} GB`
		: bytes >= 1024 * 1024
			? `${(bytes / 1024 / 1024).toFixed(1)} MB`
			: `${Math.max(1, Math.round(bytes / 1024))} KB`
}

/** The limits of the server that an upload can be held against before it is sent */
export type UploadLimits = {
	maxRequestBytes: number
	maxFileBytes: number
	maxFilesPerRequest: number
	quotaBytes: number
	usedBytes: number
	maxItems: number
	openItems: number
	quotaApplies: boolean
	uploadsOpen: boolean
	closedReason?: string | null
}

/**
 * Why an upload is sure to be refused, or `undefined` when it may go. The server decides in the
 * end, from the same numbers: this only says so before the bytes are sent, and in plain words.
 * Of the quota only what the server has counted so far is known.
 */
export function uploadProblem(
	batch: Pick<UploadBatch, 'files' | 'totalBytes' | 'label' | 'isFolder'>,
	limits: UploadLimits,
): string | undefined {
	if (!limits.uploadsOpen) return limits.closedReason || 'Uploads are closed right now'
	if (batch.files.length > limits.maxFilesPerRequest) {
		return `${batch.isFolder ? 'A folder' : 'An upload'} can hold at most ${limits.maxFilesPerRequest} files`
	}
	let biggest: File | undefined
	for (const file of batch.files) {
		if (!biggest || file.size > biggest.size) biggest = file
	}
	if (biggest && biggest.size > limits.maxFileBytes) {
		return `${biggest.name} is ${formatBytes(biggest.size)}: the biggest file you can add is ${formatBytes(limits.maxFileBytes)}`
	}
	if (batch.totalBytes > limits.maxRequestBytes) {
		return `${formatBytes(batch.totalBytes)} is more than one upload can hold (${formatBytes(limits.maxRequestBytes)}): add it in smaller parts`
	}
	if (limits.quotaApplies) {
		const room = Math.max(0, limits.quotaBytes - limits.usedBytes)
		if (batch.totalBytes > room) {
			return `There is not enough room in your inbox: ${formatBytes(room)} free, and this is ${formatBytes(batch.totalBytes)}. Send or discard some books first`
		}
		if (limits.openItems >= limits.maxItems) {
			return `Your inbox is full (${limits.maxItems} books): send or discard some first`
		}
	}
	return undefined
}

/** The age limit of a person, as the server keeps it */
export type AgeLimit = { age: number; restrictOnUnset: boolean }

/**
 * Whether an age rating would keep a book from somebody with an age limit: a rating above their
 * limit hides it, and so does no rating at all when they can only read rated books (the book's
 * series may still carry one, so that last case is a "may")
 */
export function ageRatingHides(
	ageRating: number | null | undefined,
	limit: AgeLimit | null | undefined,
): 'hidden' | 'unrated' | null {
	if (!limit) return null
	if (ageRating === null || ageRating === undefined) return limit.restrictOnUnset ? 'unrated' : null
	return ageRating > limit.age ? 'hidden' : null
}
