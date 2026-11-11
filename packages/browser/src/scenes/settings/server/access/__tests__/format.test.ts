import { formatWhen, shortAgent } from '../format'

describe('formatWhen', () => {
	it('is a dash for no moment, or one that is not a date', () => {
		expect(formatWhen(null)).toBe('–')
		expect(formatWhen(undefined)).toBe('–')
		expect(formatWhen('')).toBe('–')
		expect(formatWhen('not a date')).toBe('–')
	})

	it('writes a moment with its day and its time', () => {
		const text = formatWhen('2026-11-10T21:05:00Z')
		expect(text).toMatch(/2026/)
		expect(text).toMatch(/\d/)
		expect(text).not.toBe('–')
	})
})

describe('shortAgent', () => {
	it('is a dash for none', () => {
		expect(shortAgent('')).toBe('–')
		expect(shortAgent(null)).toBe('–')
		expect(shortAgent('   ')).toBe('–')
	})

	it('keeps a short one and cuts a long one with an ellipsis', () => {
		expect(shortAgent('Firefox')).toBe('Firefox')
		const long = 'Mozilla/5.0 '.repeat(20)
		const cut = shortAgent(long, 30)
		expect(cut).toHaveLength(30)
		expect(cut.endsWith('…')).toBe(true)
	})
})
