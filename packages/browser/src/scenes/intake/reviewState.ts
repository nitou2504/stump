import { IntakeSeriesKind, IntakeSource, IntakeState, IntakeUploadMode } from '@stump/graphql'

import type { IntakeItemData } from './queries'

/** The book formats the intake accepts (plus .zip packs of books) */
export const INTAKE_EXTENSIONS = ['epub', 'pdf', 'cbz', 'cbr', 'zip'] as const

const SOURCE_LABELS: Record<IntakeSource, string> = {
	[IntakeSource.File]: 'File',
	[IntakeSource.FileName]: 'File name',
	[IntakeSource.OpenLibrary]: 'Open Library',
	[IntakeSource.GoogleBooks]: 'Google Books',
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
	filed: IntakeItemData[]
}

/**
 * Splits the inbox into what the page shows: items being analysed, items to review
 * (needs review, failed, or with problems/duplicates), items ready to file, and items
 * already filed. The server returns them newest first; the order is kept.
 */
export function groupInbox(items: IntakeItemData[]): InboxGroups {
	const groups: InboxGroups = { analysing: [], filed: [], ready: [], toReview: [] }
	for (const item of items) {
		if (item.state === IntakeState.Discarded) continue
		if (item.state === IntakeState.Analysing) groups.analysing.push(item)
		else if (item.state === IntakeState.Filed) groups.filed.push(item)
		else if (needsAttention(item)) groups.toReview.push(item)
		else groups.ready.push(item)
	}
	return groups
}

/** The ids "Accept all ready" files: ready, nothing blocking, no duplicate warning */
export function acceptableIds(items: IntakeItemData[]) {
	return items.filter((item) => isReady(item) && !needsAttention(item)).map(({ id }) => id)
}

/** Whether the inbox should be polled (something is still being analysed or scanned) */
export function shouldPoll(items: IntakeItemData[]) {
	return items.some(
		(item) =>
			item.state === IntakeState.Analysing ||
			(item.state === IntakeState.Filed &&
				item.mediaIds.length === 0 &&
				!!item.filedAt &&
				Date.now() - new Date(item.filedAt).getTime() < 5 * 60 * 1000),
	)
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
}

/** The tags shown as chips: the suggestions first, then any extra chosen tag */
export function genreChips(item: Pick<IntakeItemData, 'proposal' | 'decision'>): GenreChip[] {
	const chosen = item.decision?.metadata.genres ?? []
	const suggestions = item.proposal?.genreSuggestions ?? []
	const suggested = new Set(suggestions.map((s) => s.value))
	return [
		...suggestions.map(({ value, confidence, reasons, libraryCount, kind }) => ({
			chosen: chosen.includes(value),
			confidence,
			kind,
			libraryCount,
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
				reasons: ['Added by you'],
				suggested: false,
				value,
			})),
	]
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
