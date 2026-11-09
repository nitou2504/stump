import { isoWeek } from '../DiscoverBooks'

describe('isoWeek', () => {
	it('names the ISO week, so the picks change on Mondays', () => {
		expect(isoWeek(new Date(2026, 10, 8))).toBe('2026-W45')
		expect(isoWeek(new Date(2026, 10, 9))).toBe('2026-W46')
	})

	it('puts the days around New Year in the week they belong to', () => {
		expect(isoWeek(new Date(2026, 0, 1))).toBe('2026-W01')
		expect(isoWeek(new Date(2027, 0, 1))).toBe('2026-W53')
		expect(isoWeek(new Date(2024, 11, 30))).toBe('2025-W01')
	})
})
