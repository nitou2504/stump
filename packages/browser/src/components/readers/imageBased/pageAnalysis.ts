import { useMemo } from 'react'
import { create } from 'zustand'
import { persist } from 'zustand/middleware'

import { type PageCrop, pageCrop, sampleSize, toLuma } from './pageCrop'
import { isWide } from './paged/pagedView'

/**
 * Looking at the pages a comic reader shows or preloads: where each page's blank margins are
 * (for 'crop borders').
 *
 * The page is looked at once, from the copy the browser already decoded (the visible `<img>`
 * or a preloaded `Image`): a small ~1 megapixel copy is made with `createImageBitmap` and
 * handed to a worker, so a page turn never waits for it. Results are kept per book and page
 * (in local storage, a few numbers per page), so a page is only looked at once.
 */

export type PageAnalysis = { crop: PageCrop }

/** Bumped when the analysis changes, so older results are recomputed */
const ANALYSIS_VERSION = 1
/** Kept for this many books (the most recently read) */
const MAX_BOOKS = 40

type AnalysisStore = {
	version: number
	books: Record<string, { at: number; pages: Record<number, PageAnalysis> }>
	store: (bookId: string, page: number, analysis: PageAnalysis) => void
}

const useAnalysisStore = create<AnalysisStore>()(
	persist(
		(set) => ({
			version: ANALYSIS_VERSION,
			books: {},
			store: (bookId, page, analysis) =>
				set((state) => {
					const books = { ...state.books }
					books[bookId] = {
						at: Date.now(),
						pages: { ...books[bookId]?.pages, [page]: analysis },
					}
					const ids = Object.keys(books)
					if (ids.length > MAX_BOOKS) {
						ids
							.sort((a, b) => books[a]!.at - books[b]!.at)
							.slice(0, ids.length - MAX_BOOKS)
							.forEach((id) => delete books[id])
					}
					return { books }
				}),
		}),
		{
			name: 'stump-page-analysis',
			version: ANALYSIS_VERSION,
			// results of an older analysis are dropped
			migrate: () => ({ version: ANALYSIS_VERSION, books: {} }) as unknown as AnalysisStore,
		},
	),
)

/** The analysed pages of a book (1-based page numbers) */
export function usePageAnalyses(bookId: string): Record<number, PageAnalysis> {
	const pages = useAnalysisStore((state) => state.books[bookId]?.pages)
	return useMemo(() => pages ?? {}, [pages])
}

type Job = {
	key: string
	bookId: string
	page: number
	image: HTMLImageElement
}

const queue: Job[] = []
const queued = new Set<string>()
let running = false
let worker: Worker | null | undefined
let nextId = 1
const waiting = new Map<number, (result: PageAnalysis | null) => void>()

const keyOf = (bookId: string, page: number) => `${bookId}:${page}`

function getWorker(): Worker | null {
	if (worker !== undefined) return worker
	try {
		const canUseWorker =
			typeof Worker !== 'undefined' &&
			typeof OffscreenCanvas !== 'undefined' &&
			typeof createImageBitmap !== 'undefined'
		worker = canUseWorker
			? new Worker(new URL('./cropWorker.ts', import.meta.url), { type: 'module' })
			: null
		worker?.addEventListener('message', (event: MessageEvent) => {
			const { id, crop, error } = event.data ?? {}
			const resolve = waiting.get(id)
			waiting.delete(id)
			resolve?.(error ? null : { crop })
		})
		worker?.addEventListener('error', () => {
			// a worker that can't start: look at pages on the main thread from now on
			worker?.terminate()
			worker = null
			waiting.forEach((resolve) => resolve(null))
			waiting.clear()
		})
	} catch {
		worker = null
	}
	return worker
}

/** On the main thread, for browsers without workers that can draw (a few ms per page) */
function analyseHere(image: HTMLImageElement, withHalves: boolean): PageAnalysis | null {
	const natural = { w: image.naturalWidth, h: image.naturalHeight }
	const sample = sampleSize(natural.w, natural.h)
	const canvas = document.createElement('canvas')
	canvas.width = sample.w
	canvas.height = sample.h
	const context = canvas.getContext('2d', { willReadFrequently: true })
	if (!context) return null
	context.fillStyle = '#fff'
	context.fillRect(0, 0, sample.w, sample.h)
	context.drawImage(image, 0, 0, sample.w, sample.h)
	const luma = toLuma(context.getImageData(0, 0, sample.w, sample.h).data, sample.w * sample.h)
	return { crop: pageCrop(luma, sample, natural, withHalves) }
}

/**
 * Looks at one decoded picture (off the main thread when the browser can), without storing
 * the result; the PDF reader's margin trim uses it too
 */
export async function analyseImage(image: HTMLImageElement): Promise<PageAnalysis | null> {
	const natural = { w: image.naturalWidth, h: image.naturalHeight }
	if (!natural.w || !natural.h) return null
	const withHalves = isWide(natural.w / natural.h)
	const target = getWorker()
	if (!target) return analyseHere(image, withHalves)
	const sample = sampleSize(natural.w, natural.h)
	const bitmap = await createImageBitmap(image, {
		resizeWidth: sample.w,
		resizeHeight: sample.h,
		resizeQuality: 'low',
	})
	const id = nextId++
	return new Promise((resolve) => {
		waiting.set(id, resolve)
		target.postMessage({ id, bitmap, natural, withHalves }, [bitmap])
	})
}

async function drain() {
	if (running) return
	running = true
	try {
		while (queue.length) {
			const job = queue.shift()!
			queued.delete(job.key)
			if (useAnalysisStore.getState().books[job.bookId]?.pages[job.page]) continue
			try {
				const result = await analyseImage(job.image)
				if (result) useAnalysisStore.getState().store(job.bookId, job.page, result)
			} catch (error) {
				console.warn('Failed to look at page', job.page, error)
			}
		}
	} finally {
		running = false
	}
}

/**
 * Asks for a loaded page (1-based) to be looked at, unless it already was. `urgent` (the page
 * on screen) goes ahead of the preloaded ones.
 */
export function analysePage(bookId: string, page: number, image: HTMLImageElement, urgent = false) {
	const key = keyOf(bookId, page)
	if (useAnalysisStore.getState().books[bookId]?.pages[page]) return
	if (queued.has(key)) {
		if (!urgent) return
		const index = queue.findIndex((job) => job.key === key)
		if (index > 0) queue.unshift(...queue.splice(index, 1))
		return
	}
	queued.add(key)
	const job = { key, bookId, page, image }
	if (urgent) queue.unshift(job)
	else queue.push(job)
	void drain()
}
