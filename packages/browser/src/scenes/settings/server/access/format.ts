import { formatDistanceToNow, intlFormat } from 'date-fns'

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

/**
 * A moment as a distance from now ("2 hours ago", "in 3 days"), in the language of the app; a dash
 * when there is none. The exact time is in the `title` of what shows it
 */
export function formatAgo(value?: string | null): string {
	if (!value) return '–'
	const date = new Date(value)
	if (Number.isNaN(date.getTime())) return '–'
	return formatDistanceToNow(date, { addSuffix: true })
}

/**
 * A short name for a device from the browser's user agent: "Chrome on Linux", "Safari on iOS".
 * The same words as the server's, for the devices the page lists (which carry the raw agent)
 */
export function agentLabel(agent?: string | null): string {
	const text = agent ?? ''
	const browser = /Edg\/|EdgA\/|EdgiOS\//.test(text)
		? 'Edge'
		: /OPR\/|Opera/.test(text)
			? 'Opera'
			: text.includes('SamsungBrowser')
				? 'Samsung Internet'
				: /Firefox\/|FxiOS\//.test(text)
					? 'Firefox'
					: /Chrome\/|CriOS\/|Chromium\//.test(text)
						? 'Chrome'
						: text.includes('Safari/') && text.includes('Version/')
							? 'Safari'
							: ''
	const system = text.includes('Android')
		? 'Android'
		: /iPhone|iPad|iPod/.test(text)
			? 'iOS'
			: text.includes('Windows')
				? 'Windows'
				: text.includes('CrOS')
					? 'ChromeOS'
					: /Macintosh|Mac OS X/.test(text)
						? 'macOS'
						: /Linux|X11/.test(text)
							? 'Linux'
							: ''
	if (!browser && !system) return 'Browser'
	if (!browser) return `Browser on ${system}`
	return system ? `${browser} on ${system}` : browser
}
