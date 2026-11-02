import {
	type ReaderKind,
	type ReadingBarItem,
	type ReadingBarShow,
	type ReadingProgressDisplay,
	type ReadingProgressPreferences,
} from '@stump/client'
import { useCallback, useMemo } from 'react'

import { useReaderStore } from '@/stores'

/**
 * Each reader's reading progress (the small number in a corner, or the reading bar at the
 * bottom of the page, like KOReader's footer), and the maths of the bar: where its chapter
 * ticks go, what's left of the chapter and the words under the rule.
 */

export type ReadingProgress = Required<ReadingProgressPreferences>

/** What each reader shows until it's changed: the EPUB its bar, the others their number */
export const READING_PROGRESS_DEFAULTS: Record<ReaderKind, ReadingProgress> = {
	epub: { display: 'bar', items: ['chapter', 'pagesLeft', 'percent'], show: 'always' },
	pdf: { display: 'number', items: ['chapter', 'page', 'pagesLeft'], show: 'always' },
	comic: { display: 'number', items: ['page'], show: 'always' },
}

/** The bar items each reader can show (a comic has no chapters or text) */
export const READING_BAR_ITEMS_FOR: Record<ReaderKind, ReadingBarItem[]> = {
	epub: ['chapter', 'page', 'pagesLeft', 'percent', 'clock'],
	pdf: ['chapter', 'page', 'pagesLeft', 'percent', 'clock'],
	comic: ['page', 'percent', 'clock'],
}

/** A reader's reading progress settings, with its defaults filled in */
export function resolveReadingProgress(
	kind: ReaderKind,
	stored: ReadingProgressPreferences | undefined,
): ReadingProgress {
	const defaults = READING_PROGRESS_DEFAULTS[kind]
	const allowed = READING_BAR_ITEMS_FOR[kind]
	return {
		display: stored?.display ?? defaults.display,
		items: (stored?.items ?? defaults.items).filter((item) => allowed.includes(item)),
		show: stored?.show ?? defaults.show,
	}
}

/** A reader's reading progress, and a setter that keeps the other readers' as they are */
export function useReadingProgress(kind: ReaderKind) {
	const stored = useReaderStore((state) => state.settings.readingProgress?.[kind])
	const all = useReaderStore((state) => state.settings.readingProgress)
	const setSettings = useReaderStore((state) => state.setSettings)
	const progress = useMemo(() => resolveReadingProgress(kind, stored), [kind, stored])
	const setProgress = useCallback(
		(change: Partial<ReadingProgress>) =>
			setSettings({
				readingProgress: { ...all, [kind]: { ...all?.[kind], ...change } },
			}),
		[all, kind, setSettings],
	)
	return { ...progress, setProgress }
}

export type { ReaderKind, ReadingBarItem, ReadingBarShow, ReadingProgressDisplay }

// The rule's maths --------------------------------------------------------------------------------

/** Ticks closer than this (CSS px) are thinned: a book with a hundred chapters stays legible */
export const MIN_TICK_GAP = 6

/**
 * The chapter ticks to draw on a rule `width` px wide: the chapters' starts (0..1, sorted),
 * without the book's start (the rule's own end) and keeping only ticks at least
 * `MIN_TICK_GAP` px from the last one kept
 */
export function thinTicks(starts: number[], width: number, minGap = MIN_TICK_GAP): number[] {
	const kept: number[] = []
	let last = 0
	for (const start of [...starts].sort((a, b) => a - b)) {
		if (start <= 0 || start >= 1) continue
		if ((start - last) * width < minGap) continue
		kept.push(start)
		last = start
	}
	return kept
}

/** The chapter (from its start to the next one's, 0..1) around a position */
export function chapterAround(starts: number[], position: number): [number, number] {
	const sorted = [...new Set([0, ...starts.filter((start) => start > 0 && start < 1)])].sort(
		(a, b) => a - b,
	)
	let from = 0
	let to = 1
	for (const start of sorted) {
		if (start <= position) from = start
		else {
			to = start
			break
		}
	}
	return [from, to]
}

export type ChapterPages = {
	/** The page (screen) in the chapter, from 1 */
	page: number
	/** The chapter's pages (screens) */
	pages: number
	/** The pages after this one in the chapter */
	left: number
}

/**
 * The pages of the chapter on screen, from the book frame's scroller: in pages that turn,
 * how many screens wide the chapter is (a screen is one page, or two in a spread) and which
 * one shows; when scrolling, how many screen heights tall
 */
export function chapterPages({
	scrollSize,
	viewportSize,
	offset,
}: {
	/** The chapter's scroll width (pages that turn) or height (scrolling) */
	scrollSize: number
	/** The screen's width or height */
	viewportSize: number
	/** How far the chapter is scrolled (any sign, for right-to-left books) */
	offset: number
}): ChapterPages | null {
	if (viewportSize <= 0 || scrollSize <= 0) return null
	const pages = Math.max(1, Math.round(scrollSize / viewportSize))
	const page = Math.min(pages, Math.max(1, Math.floor(Math.abs(offset) / viewportSize + 0.5) + 1))
	return { page, pages, left: pages - page }
}

/**
 * The labels from the top of a contents tree down to the entry for `href`: the chapter's
 * part and its own title (`Book III`, `Chapter 1`). Matching is left to the caller.
 */
export function tocPath<T extends { label: string; children: T[] }>(
	toc: T[],
	matches: (item: T) => boolean,
): string[] {
	for (const item of toc) {
		if (matches(item)) return [item.label.trim()]
		const below = tocPath(item.children, matches)
		if (below.length) return [item.label.trim(), ...below]
	}
	return []
}

/** The chapter's name for the bar: its own title, after its part's when it has one */
export function chapterLabel(path: string[]): string | undefined {
	if (!path.length) return undefined
	return path.slice(-2).join(' · ')
}
