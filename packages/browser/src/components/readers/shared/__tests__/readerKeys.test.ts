import { isReaderSearchShortcut, nextDayNightTheme } from '../readerKeys'

const key = (k: string, modifiers: Partial<KeyboardEvent> = {}) => ({
	key: k,
	ctrlKey: false,
	metaKey: false,
	altKey: false,
	shiftKey: false,
	...modifiers,
})

describe('isReaderSearchShortcut', () => {
	it('takes Ctrl/Cmd+F and / (instead of the browser find bar and quick find)', () => {
		expect(isReaderSearchShortcut(key('f', { ctrlKey: true }))).toBe(true)
		expect(isReaderSearchShortcut(key('F', { metaKey: true }))).toBe(true)
		expect(isReaderSearchShortcut(key('/'))).toBe(true)
		// the old shortcut stays as an alias
		expect(isReaderSearchShortcut(key('k', { ctrlKey: true }))).toBe(true)
	})

	it('leaves other keys alone', () => {
		expect(isReaderSearchShortcut(key('f'))).toBe(false)
		expect(isReaderSearchShortcut(key('f', { altKey: true, ctrlKey: true }))).toBe(false)
		expect(isReaderSearchShortcut(key('/', { altKey: true }))).toBe(false)
	})
})

describe('nextDayNightTheme', () => {
	it('switches to the opposite of what is shown', () => {
		expect(nextDayNightTheme('light', false)).toBe('dark')
		expect(nextDayNightTheme('sepia', false)).toBe('dark')
		expect(nextDayNightTheme('black', false)).toBe('light')
		expect(nextDayNightTheme('auto', true)).toBe('light')
		expect(nextDayNightTheme('auto', false)).toBe('dark')
	})
})

describe('isFullscreenKey', () => {
	it('is a plain F11', async () => {
		const { isFullscreenKey } = await import('../useReaderChrome')
		const key = (extra: Partial<KeyboardEvent>) =>
			({
				key: 'F11',
				ctrlKey: false,
				metaKey: false,
				altKey: false,
				shiftKey: false,
				...extra,
			}) as KeyboardEvent
		expect(isFullscreenKey(key({}))).toBe(true)
		expect(isFullscreenKey(key({ ctrlKey: true }))).toBe(false)
		expect(isFullscreenKey(key({ key: 'F10' }))).toBe(false)
	})
})
