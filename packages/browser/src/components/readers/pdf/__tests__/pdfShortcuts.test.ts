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

describe('resolvePdfShortcut on a zoomed page', () => {
	it('turns pages with the arrows when the page fits', () => {
		expect(resolvePdfShortcut(key('ArrowRight'), paged)).toEqual({ type: 'next' })
		expect(resolvePdfShortcut(key('ArrowLeft'), paged)).toEqual({ type: 'previous' })
	})

	it('moves around a zoomed page with the arrows and Space', () => {
		const zoomed = { ...paged, zoomedPage: true }
		expect(resolvePdfShortcut(key('ArrowRight'), zoomed)).toEqual({
			type: 'pan',
			direction: { dx: 1, dy: 0 },
		})
		expect(resolvePdfShortcut(key('ArrowUp'), zoomed)).toEqual({
			type: 'pan',
			direction: { dx: 0, dy: -1 },
		})
		expect(resolvePdfShortcut(key(' ', { shiftKey: true }), zoomed)).toEqual({
			type: 'pan',
			direction: { dx: 0, dy: -1 },
			screen: true,
		})
	})

	it('still turns pages with Page Up / Page Down when zoomed', () => {
		const zoomed = { ...paged, zoomedPage: true }
		expect(resolvePdfShortcut(key('PageDown'), zoomed)).toEqual({ type: 'next' })
		expect(resolvePdfShortcut(key('PageUp'), zoomed)).toEqual({ type: 'previous' })
	})

	it('leaves continuous scrolling as it was', () => {
		const continuous = { ...paged, continuous: true, zoomedPage: true }
		expect(resolvePdfShortcut(key('ArrowRight'), continuous)).toEqual({ type: 'next' })
		expect(resolvePdfShortcut(key(' '), continuous)).toEqual({ type: 'screenDown' })
	})
})
