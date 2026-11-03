import { describe, expect, it } from 'vitest'

import {
	books,
	countdownLabel,
	daysLeft,
	formatBytes,
	fuseTicks,
	heldFileLabel,
	isImminent,
	retentionLabel,
	retentionOptions,
} from '../countdown'

describe('daysLeft', () => {
	const now = new Date(2026, 10, 3, 11, 6) // 3 Nov, 11:06 local

	it('counts calendar days to the purge', () => {
		expect(daysLeft(new Date(2026, 10, 17, 23, 0).toISOString(), now)).toBe(14)
		expect(daysLeft(new Date(2026, 10, 4, 0, 30).toISOString(), now)).toBe(1)
		expect(daysLeft(new Date(2026, 10, 3, 23, 0).toISOString(), now)).toBe(0)
	})

	it('is 0 when overdue and null when books are kept until deleted by hand', () => {
		expect(daysLeft(new Date(2026, 10, 1).toISOString(), now)).toBe(0)
		expect(daysLeft(null, now)).toBeNull()
		expect(daysLeft(undefined, now)).toBeNull()
	})
})

describe('countdownLabel', () => {
	it('says when the book goes', () => {
		expect(countdownLabel(14)).toBe('Deleted permanently in 14 days')
		expect(countdownLabel(1)).toBe('Deleted permanently tomorrow')
		expect(countdownLabel(0)).toBe('Deleted permanently today')
		expect(countdownLabel(null)).toBe('Kept until deleted by hand')
	})

	it('flags the last day and the day before', () => {
		expect(isImminent(0)).toBe(true)
		expect(isImminent(1)).toBe(true)
		expect(isImminent(2)).toBe(false)
		expect(isImminent(null)).toBe(false)
	})
})

describe('fuseTicks', () => {
	it('draws the days spent, then the days left, one tick a day', () => {
		const ticks = fuseTicks(12, 14)
		expect(ticks).toHaveLength(14)
		expect(ticks.slice(0, 2)).toEqual(['spent', 'spent'])
		expect(ticks.filter((tick) => tick === 'left')).toHaveLength(12)
		expect(ticks[13]).toBe('left')
	})

	it('is full on the first day and burnt out on the last', () => {
		expect(fuseTicks(14, 14).every((tick) => tick === 'left')).toBe(true)
		// the purge can be a day after the retention ends (it runs once a day)
		expect(fuseTicks(15, 14).every((tick) => tick === 'left')).toBe(true)
		expect(fuseTicks(0, 14).every((tick) => tick === 'spent')).toBe(true)
	})

	it('uses a 30-tick scale for long retentions and draws nothing when kept forever', () => {
		const ticks = fuseTicks(90, 180)
		expect(ticks).toHaveLength(30)
		expect(ticks.filter((tick) => tick === 'left')).toHaveLength(15)
		expect(fuseTicks(null, 14)).toEqual([])
		expect(fuseTicks(3, 0)).toEqual([])
	})
})

describe('retention options', () => {
	it('labels the presets and keeps a value set elsewhere', () => {
		expect(retentionLabel(0)).toBe('Never')
		expect(retentionLabel(1)).toBe('1 day')
		expect(retentionLabel(14)).toBe('14 days')
		expect(retentionOptions(14).map((option) => option.value)).toContain('14')
		const custom = retentionOptions(21).map((option) => option.value)
		expect(custom).toContain('21')
		expect(custom.indexOf('21')).toBe(custom.indexOf('14') + 1)
	})
})

describe('formatting', () => {
	it('formats sizes and counts', () => {
		expect(formatBytes(512)).toBe('512 B')
		expect(formatBytes(1536)).toBe('1.5 KB')
		expect(formatBytes(25 * 1024 * 1024)).toBe('25 MB')
		expect(formatBytes(null)).toBe('')
		expect(books(1)).toBe('1 book')
		expect(books(3)).toBe('3 books')
	})

	it('says what a held file is', () => {
		expect(heldFileLabel('GREY16')).toBe('Original before 16 greys')
		expect(heldFileLabel('PDF', 'OCR+SHRINK')).toBe('Original before OCR and shrinking')
		expect(heldFileLabel('PDF', 'SHRINK')).toBe('Original before shrinking')
		expect(heldFileLabel('PDF', null)).toBe('Original before post-processing')
		expect(heldFileLabel('PUT_BACK', 'OCR')).toBe('Version replaced when its original was put back')
	})
})
