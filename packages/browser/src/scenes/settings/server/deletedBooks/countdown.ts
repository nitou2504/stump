import { differenceInCalendarDays } from 'date-fns'

/** The retention presets offered in the settings; 0 keeps books until they are deleted by hand */
export const RETENTION_PRESETS = [0, 1, 3, 7, 14, 30, 60, 90, 180, 365]

export const retentionLabel = (days: number) =>
	days === 0 ? 'Never' : days === 1 ? '1 day' : `${days} days`

/** The options of the retention select: the presets, plus the current value if it is another */
export const retentionOptions = (current: number) =>
	(RETENTION_PRESETS.includes(current)
		? RETENTION_PRESETS
		: [...RETENTION_PRESETS, current].sort((a, b) => a - b)
	).map((days) => ({ label: retentionLabel(days), value: String(days) }))

/**
 * Whole days, on the calendar, until the automatic purge deletes a book permanently: 0 when it
 * happens today (or is overdue), null when the book is kept until deleted by hand
 */
export function daysLeft(purgeAt: string | null | undefined, now: Date): number | null {
	if (!purgeAt) return null
	return Math.max(0, differenceInCalendarDays(new Date(purgeAt), now))
}

/** "Deleted permanently in 12 days" and its variants */
export function countdownLabel(days: number | null): string {
	if (days === null) return 'Kept until deleted by hand'
	if (days === 0) return 'Deleted permanently today'
	if (days === 1) return 'Deleted permanently tomorrow'
	return `Deleted permanently in ${days} days`
}

export type FuseTick = 'left' | 'spent'

/**
 * The countdown drawn as a fuse, one tick per day of the retention read left to right: the days
 * already spent in Deleted books, then the days left before the purge. Retentions longer than a
 * month are drawn on a 30-tick scale
 */
export function fuseTicks(days: number | null, retentionDays: number): FuseTick[] {
	if (days === null || retentionDays <= 0) return []
	const scale = Math.min(retentionDays, 30)
	const left = Math.min(scale, Math.ceil((Math.min(days, retentionDays) / retentionDays) * scale))
	return Array.from({ length: scale }, (_, i) => (i < scale - left ? 'spent' : 'left'))
}

/** Whether a book is about to go: today or tomorrow */
export const isImminent = (days: number | null) => days !== null && days <= 1

export function formatBytes(bytes: number | null | undefined): string {
	if (bytes == null) return ''
	if (bytes < 1024) return `${bytes} B`
	const units = ['KB', 'MB', 'GB', 'TB']
	let value = bytes / 1024
	let unit = 0
	while (value >= 1024 && unit < units.length - 1) {
		value /= 1024
		unit += 1
	}
	return `${value >= 10 ? Math.round(value) : value.toFixed(1)} ${units[unit]}`
}

/** "1 book", "3 books" */
export const books = (count: number) => (count === 1 ? '1 book' : `${count} books`)

/** The quarantine day folder a book deleted now goes to, as the server names it (UTC date) */
export function todaysQuarantineFolder(quarantineDir: string, now: Date): string {
	const day = now.toISOString().slice(0, 10)
	return `${quarantineDir.replace(/\/+$/, '')}/deleted/${day}/`
}
