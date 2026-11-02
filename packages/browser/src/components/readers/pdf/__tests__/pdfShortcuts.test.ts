import { resolvePdfShortcut } from '../pdfShortcuts'

const key = (key: string, extra: Partial<KeyboardEvent> = {}) =>
	({
		key,
		ctrlKey: false,
		metaKey: false,
		altKey: false,
		shiftKey: false,
		...extra,
	}) as KeyboardEvent

const paged = { hasTarget: false, targetIsAnnotation: false, continuous: false }

const line = (dx: -1 | 0 | 1, dy: -1 | 0 | 1) => ({
	type: 'scroll',
	action: { type: 'line', dx, dy },
})
const screen = (dy: -1 | 1) => ({ type: 'scroll', action: { type: 'screen', dy } })

describe('resolvePdfShortcut, pages that turn', () => {
	it('turns pages with the arrows and Page Up / Page Down when the page fits', () => {
		expect(resolvePdfShortcut(key('ArrowRight'), paged)).toEqual({ type: 'next' })
		expect(resolvePdfShortcut(key('ArrowLeft'), paged)).toEqual({ type: 'previous' })
		expect(resolvePdfShortcut(key('PageDown'), paged)).toEqual({ type: 'next' })
		expect(resolvePdfShortcut(key('PageUp'), paged)).toEqual({ type: 'previous' })
	})

	it('moves down a page taller than the screen with ↑ ↓, and steps through it with Space', () => {
		expect(resolvePdfShortcut(key('ArrowDown'), paged)).toEqual(line(0, 1))
		expect(resolvePdfShortcut(key('ArrowUp'), paged)).toEqual(line(0, -1))
		expect(resolvePdfShortcut(key(' '), paged)).toEqual({ type: 'step', delta: 1 })
		expect(resolvePdfShortcut(key(' ', { shiftKey: true }), paged)).toEqual({
			type: 'step',
			delta: -1,
		})
	})
})

describe('resolvePdfShortcut on a zoomed page', () => {
	const zoomed = { ...paged, zoomedPage: true }

	it('moves around it with the arrows and Space, which never turn the page', () => {
		expect(resolvePdfShortcut(key('ArrowRight'), zoomed)).toEqual(line(1, 0))
		expect(resolvePdfShortcut(key('ArrowLeft'), zoomed)).toEqual(line(-1, 0))
		expect(resolvePdfShortcut(key('ArrowUp'), zoomed)).toEqual(line(0, -1))
		expect(resolvePdfShortcut(key('ArrowDown'), zoomed)).toEqual(line(0, 1))
		expect(resolvePdfShortcut(key(' '), zoomed)).toEqual(screen(1))
		expect(resolvePdfShortcut(key(' ', { shiftKey: true }), zoomed)).toEqual(screen(-1))
	})

	it('turns pages only with Page Up / Page Down', () => {
		expect(resolvePdfShortcut(key('PageDown'), zoomed)).toEqual({ type: 'next' })
		expect(resolvePdfShortcut(key('PageUp'), zoomed)).toEqual({ type: 'previous' })
		expect(resolvePdfShortcut(key('Home'), zoomed)).toEqual({ type: 'first' })
		expect(resolvePdfShortcut(key('End'), zoomed)).toEqual({ type: 'last' })
	})
})

describe('resolvePdfShortcut, pages that scroll', () => {
	const continuous = { ...paged, continuous: true }

	it('scrolls a line with the arrows and a screen with Space and Page Up / Page Down', () => {
		expect(resolvePdfShortcut(key('ArrowDown'), continuous)).toEqual(line(0, 1))
		expect(resolvePdfShortcut(key('ArrowUp'), continuous)).toEqual(line(0, -1))
		// sideways: only moves when a zoomed page is wider than the screen
		expect(resolvePdfShortcut(key('ArrowRight'), continuous)).toEqual(line(1, 0))
		expect(resolvePdfShortcut(key('PageDown'), continuous)).toEqual(screen(1))
		expect(resolvePdfShortcut(key('PageUp'), continuous)).toEqual(screen(-1))
		expect(resolvePdfShortcut(key(' '), continuous)).toEqual(screen(1))
		expect(resolvePdfShortcut(key(' ', { shiftKey: true }), continuous)).toEqual(screen(-1))
		expect(resolvePdfShortcut(key('Home'), continuous)).toEqual({ type: 'first' })
		expect(resolvePdfShortcut(key('End'), continuous)).toEqual({ type: 'last' })
	})

	it('leaves Shift+arrows to the text selection', () => {
		expect(resolvePdfShortcut(key('ArrowDown', { shiftKey: true }), continuous)).toBeNull()
	})
})
