import { intlFormat } from 'date-fns'

/** A moment as a short, local date and time ("Nov 10, 2026, 9:05 PM"); a dash when there is none */
export function formatWhen(value?: string | null): string {
	if (!value) return '–'
	const date = new Date(value)
	if (Number.isNaN(date.getTime())) return '–'
	return intlFormat(date, {
		month: 'short',
		day: 'numeric',
		year: 'numeric',
		hour: 'numeric',
		minute: '2-digit',
	})
}

/**
 * The part of a browser's description that tells the owner which device it was: the user agent
 * as the server keeps it is long, so it is cut to what fits a row (the whole is in the title)
 */
export function shortAgent(agent?: string | null, max = 60): string {
	const text = (agent ?? '').trim()
	if (!text) return '–'
	return text.length > max ? `${text.slice(0, max - 1)}…` : text
}
