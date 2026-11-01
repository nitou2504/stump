import { AnnotationStyle } from '@stump/graphql'

import { chapterTitleForPage, flattenOutline, pageLabel, resolvePageInput } from '../outline'
import { resolvePdfShortcut } from '../pdfShortcuts'
import { isDarkTheme, pdfThemeLook, resolvePdfTheme } from '../pdfThemes'

describe('outline', () => {
	const outline = [
		{
			title: 'Part I',
			page: 5,
			children: [
				{ title: 'Chapter 1', page: 5, children: [] },
				{ title: 'Chapter 2', page: 20, children: [] },
			],
		},
		{ title: 'Untitled', page: null, children: [] },
		{ title: 'Part II', page: 40, children: [] },
	]
	const flat = flattenOutline(outline)

	it('flattens with depths', () => {
		expect(flat.map((entry) => [entry.title, entry.depth])).toEqual([
			['Part I', 0],
			['Chapter 1', 1],
			['Chapter 2', 1],
			['Untitled', 0],
			['Part II', 0],
		])
	})

	it('finds the innermost entry containing a page', () => {
		expect(chapterTitleForPage(flat, 1)).toBe('')
		expect(chapterTitleForPage(flat, 5)).toBe('Chapter 1')
		expect(chapterTitleForPage(flat, 25)).toBe('Chapter 2')
		expect(chapterTitleForPage(flat, 40)).toBe('Part II')
	})

	it('labels pages and resolves typed pages', () => {
		const layout = { labels: ['i', 'ii', '1', '2'], pageCount: 4 }
		expect(pageLabel(layout, 2)).toBe('ii')
		expect(pageLabel({ labels: null }, 2)).toBe('2')
		expect(resolvePageInput(layout, 'II')).toBe(2)
		expect(resolvePageInput(layout, '1')).toBe(3) // the label "1" is page 3
		expect(resolvePageInput({ labels: null, pageCount: 4 }, '4')).toBe(4)
		expect(resolvePageInput({ labels: null, pageCount: 4 }, '5')).toBeNull()
		expect(resolvePageInput({ labels: null, pageCount: 4 }, 'x')).toBeNull()
	})
})

describe('resolvePdfShortcut', () => {
	const key = (k: string, modifiers: Partial<KeyboardEvent> = {}) => ({
		key: k,
		ctrlKey: false,
		metaKey: false,
		altKey: false,
		shiftKey: false,
		...modifiers,
	})
	const idle = { hasTarget: false, targetIsAnnotation: false, continuous: true }
	const selected = { hasTarget: true, targetIsAnnotation: false, continuous: true }

	it('keeps the annotation shortcuts', () => {
		expect(resolvePdfShortcut(key('h'), selected)).toEqual({
			type: 'style',
			style: AnnotationStyle.Highlight,
		})
		expect(resolvePdfShortcut(key('b'), idle)).toEqual({ type: 'bookmark' })
		expect(resolvePdfShortcut(key('?'), idle)).toEqual({ type: 'help' })
		// letters do nothing without a selection
		expect(resolvePdfShortcut(key('h'), idle)).toBeNull()
	})

	it('turns pages and scrolls screens', () => {
		expect(resolvePdfShortcut(key('ArrowRight'), idle)).toEqual({ type: 'next' })
		expect(resolvePdfShortcut(key('PageUp'), idle)).toEqual({ type: 'previous' })
		expect(resolvePdfShortcut(key(' '), idle)).toEqual({ type: 'screenDown' })
		expect(resolvePdfShortcut(key(' ', { shiftKey: true }), idle)).toEqual({ type: 'screenUp' })
		expect(resolvePdfShortcut(key(' '), { ...idle, continuous: false })).toEqual({ type: 'next' })
		expect(resolvePdfShortcut(key('End'), idle)).toEqual({ type: 'last' })
	})

	it('zooms, fits, goes to a page and searches', () => {
		expect(resolvePdfShortcut(key('+'), idle)).toEqual({ type: 'zoomIn' })
		expect(resolvePdfShortcut(key('-'), idle)).toEqual({ type: 'zoomOut' })
		expect(resolvePdfShortcut(key('0'), idle)).toEqual({ type: 'zoomReset' })
		expect(resolvePdfShortcut(key('f'), selected)).toEqual({ type: 'toggleFit' })
		expect(resolvePdfShortcut(key('G'), idle)).toEqual({ type: 'goToPage' })
		expect(resolvePdfShortcut(key('k', { ctrlKey: true }), idle)).toEqual({ type: 'search' })
		expect(resolvePdfShortcut(key('K', { metaKey: true }), selected)).toEqual({ type: 'search' })
	})

	it('leaves other modified keys alone', () => {
		expect(resolvePdfShortcut(key('ArrowRight', { altKey: true }), idle)).toBeNull()
		expect(resolvePdfShortcut(key('+', { ctrlKey: true }), idle)).toBeNull()
		expect(resolvePdfShortcut(key('x'), idle)).toBeNull()
	})
})

describe('pdf themes', () => {
	it('resolves auto with the app theme', () => {
		expect(resolvePdfTheme('auto', true)).toBe('dark')
		expect(resolvePdfTheme(undefined, false)).toBe('light')
		expect(resolvePdfTheme('sepia', true)).toBe('sepia')
		expect(isDarkTheme('black')).toBe(true)
	})

	it('inverts pages in dark themes unless turned off', () => {
		expect(pdfThemeLook('light', true).imageFilter).toBeUndefined()
		expect(pdfThemeLook('dark', true).imageFilter).toContain('invert(1)')
		expect(pdfThemeLook('black', true).highlightBlend).toBe('screen')
		const plainDark = pdfThemeLook('dark', false)
		expect(plainDark.imageFilter).toBeUndefined()
		expect(plainDark.highlightBlend).toBe('multiply')
		expect(pdfThemeLook('sepia', true).tint).toBe('#f4ecd8')
	})
})
