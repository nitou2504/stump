import type { PdfPageText, PdfTextRun } from '@stump/sdk'
import { useMemo } from 'react'
import { create } from 'zustand'
import { persist } from 'zustand/middleware'

import { analyseImage } from '../imageBased/pageAnalysis'
import type { FractionRect, PageSize } from './geometry'

/**
 * Trim margins: each PDF page is shown without its blank margins.
 *
 * A page is looked at once, from the picture the reader already loaded, with the comic
 * reader's border detection (`imageBased/pageCrop.ts`, at most 15% a side). Its text lines
 * are kept inside the box, so a faint line or a page number near the edge is never cut,
 * and a little paper is left around the content. The box is a fraction of the full page and
 * is kept per book and document version (local storage), so a page is looked at only once.
 * Everything else (text layer, annotations, bookmarks, search hits) stays in full-page
 * fractions: the full page is drawn inside a smaller frame.
 */

/** Paper left around the content, as a share of the page */
export const TRIM_PAD = 0.01
/** A trim smaller than this on every side isn't worth changing the page's size for */
export const MIN_TRIM = 0.01

const round = (value: number) => Math.round(value * 10000) / 10000

/** A text run worth keeping in view: at least two letters or digits (not a stray mark) */
function isWordRun(run: PdfTextRun): boolean {
	return (run[4].match(/[\p{L}\p{N}]/gu)?.length ?? 0) >= 2 && run[2] > 0 && run[3] > 0
}

/** The box around a page's text lines (page fractions), or `null` without text */
export function textBox(runs: readonly PdfTextRun[] | undefined | null): FractionRect | null {
	const words = (runs ?? []).filter(isWordRun)
	if (!words.length) return null
	const left = Math.min(...words.map((run) => run[0]))
	const top = Math.min(...words.map((run) => run[1]))
	const right = Math.max(...words.map((run) => run[0] + run[2]))
	const bottom = Math.max(...words.map((run) => run[1] + run[3]))
	return { x: left, y: top, width: right - left, height: bottom - top }
}

/**
 * The part of the page to show: the content the picture shows (`content`, `null` when
 * there's nothing to trim), grown to hold the text lines and a little paper. `null` keeps
 * the whole page.
 */
export function trimRect(
	content: FractionRect | null,
	text: FractionRect | null,
	pad = TRIM_PAD,
): FractionRect | null {
	if (!content) return null
	let left = content.x
	let top = content.y
	let right = content.x + content.width
	let bottom = content.y + content.height
	if (text) {
		left = Math.min(left, text.x)
		top = Math.min(top, text.y)
		right = Math.max(right, text.x + text.width)
		bottom = Math.max(bottom, text.y + text.height)
	}
	left = Math.max(0, left - pad)
	top = Math.max(0, top - pad)
	right = Math.min(1, right + pad)
	bottom = Math.min(1, bottom + pad)
	const worth = left > MIN_TRIM || top > MIN_TRIM || 1 - right > MIN_TRIM || 1 - bottom > MIN_TRIM
	if (!worth || right - left <= 0.1 || bottom - top <= 0.1) return null
	return {
		x: round(left),
		y: round(top),
		width: round(right - left),
		height: round(bottom - top),
	}
}

/** The size of the part of the page shown, in the page's points */
export function trimmedPage(page: PageSize, trim: FractionRect | null | undefined): PageSize {
	return trim ? [page[0] * trim.width, page[1] * trim.height] : page
}

/** A point down the full page (0-1) as a share of the trimmed page shown */
export function shownY(trim: FractionRect | null | undefined, y: number): number {
	return trim ? (y - trim.y) / trim.height : y
}

// Results ---------------------------------------------------------------------------------------

/** Bumped when the trim changes, so older results are recomputed */
const TRIM_VERSION = 1
/** Kept for this many books (the most recently read) */
const MAX_BOOKS = 40

/** Per page (1-based): the part to show, `null` for the whole page */
export type PageTrims = Record<number, FractionRect | null>

type TrimStore = {
	books: Record<string, { at: number; pages: PageTrims }>
	store: (bookKey: string, page: number, trim: FractionRect | null) => void
}

const useTrimStore = create<TrimStore>()(
	persist(
		(set) => ({
			books: {},
			store: (bookKey, page, trim) =>
				set((state) => {
					const books = { ...state.books }
					books[bookKey] = { at: Date.now(), pages: { ...books[bookKey]?.pages, [page]: trim } }
					const keys = Object.keys(books)
					if (keys.length > MAX_BOOKS) {
						keys
							.sort((a, b) => books[a]!.at - books[b]!.at)
							.slice(0, keys.length - MAX_BOOKS)
							.forEach((key) => delete books[key])
					}
					return { books }
				}),
		}),
		{
			name: 'stump-pdf-trim',
			version: TRIM_VERSION,
			partialize: (state) => ({ books: state.books }),
			// results of an older trim are dropped
			migrate: () => ({ books: {} }) as unknown as TrimStore,
		},
	),
)

/** The key results are kept under: the book and its document version */
export const trimKey = (bookId: string, v: string) => `${bookId}@${v}`

/** The pages of a book looked at so far */
export function usePageTrims(bookKey: string): PageTrims {
	const pages = useTrimStore((state) => state.books[bookKey]?.pages)
	return useMemo(() => pages ?? {}, [pages])
}

/** Whether a page was looked at already */
export function hasTrim(bookKey: string, page: number): boolean {
	return page in (useTrimStore.getState().books[bookKey]?.pages ?? {})
}

type Job = {
	key: string
	bookKey: string
	page: number
	image: HTMLImageElement
	loadText: (() => Promise<PdfPageText | null | undefined>) | null
}

const queue: Job[] = []
const queued = new Set<string>()
let running = false

async function trimOf(job: Job): Promise<FractionRect | null | undefined> {
	const { naturalWidth: w, naturalHeight: h } = job.image
	if (!w || !h) return undefined
	const [analysis, text] = await Promise.all([
		analyseImage(job.image),
		job.loadText ? job.loadText().catch(() => null) : Promise.resolve(null),
	])
	if (!analysis) return undefined
	const box = analysis.crop.whole
	const content = box ? { x: box.x / w, y: box.y / h, width: box.w / w, height: box.h / h } : null
	return trimRect(content, textBox(text?.runs))
}

async function drain() {
	if (running) return
	running = true
	try {
		while (queue.length) {
			const job = queue.shift()!
			queued.delete(job.key)
			if (hasTrim(job.bookKey, job.page)) continue
			try {
				const trim = await trimOf(job)
				if (trim !== undefined) useTrimStore.getState().store(job.bookKey, job.page, trim)
			} catch (error) {
				console.warn('Failed to trim page', job.page, error)
			}
		}
	} finally {
		running = false
	}
}

/**
 * Asks for a loaded page picture (1-based page) to be looked at, unless it already was.
 * `loadText` gives the page's text (from the cache when the reader has it), `null` for a PDF
 * without text. `urgent` (a page on screen) goes ahead of the prefetched ones.
 */
export function trimPage(
	bookKey: string,
	page: number,
	image: HTMLImageElement,
	loadText: Job['loadText'],
	urgent = false,
) {
	if (hasTrim(bookKey, page)) return
	const key = `${bookKey}:${page}`
	if (queued.has(key)) {
		if (!urgent) return
		const index = queue.findIndex((job) => job.key === key)
		if (index > 0) queue.unshift(...queue.splice(index, 1))
		return
	}
	queued.add(key)
	const job = { key, bookKey, page, image, loadText }
	if (urgent) queue.unshift(job)
	else queue.push(job)
	void drain()
}
