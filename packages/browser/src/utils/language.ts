/**
 * A book's language as the short code its chips show (`ES`, `EN`, `JA`) and, for their
 * tooltips, the language's name in the reader's language. Any language works: what the
 * metadata stores (`es`, `ES`, `es-ES`, `es_ES`, `spa`) is reduced to its two-letter code
 * when there is one (`spa` → `es`, as browsers canonicalize language tags), else kept as its
 * three-letter code (`ast`). Undetermined (`und`) or unreadable values show nothing.
 */

/** The book's language code, lowercase (`es`, `en`, `ast`), or null */
export function bookLanguageCode(value?: string | null): string | null {
	const raw = value?.trim().replace(/_/g, '-')
	if (!raw) return null
	let code: string
	try {
		// `language` can be missing for odd tags in some engines
		code = new Intl.Locale(raw).language ?? ''
	} catch {
		code = raw.split('-')[0] ?? ''
	}
	code = code.toLowerCase()
	if (!/^[a-z]{2,3}$/.test(code) || code === 'und' || code === 'zxx' || code === 'mis') {
		return null
	}
	return code
}

const displayNames = new Map<string, Intl.DisplayNames | null>()

function namesFor(locale?: string): Intl.DisplayNames | null {
	const key = locale ?? ''
	if (!displayNames.has(key)) {
		try {
			displayNames.set(
				key,
				new Intl.DisplayNames(locale ? [locale, 'en'] : undefined, { type: 'language' }),
			)
		} catch {
			displayNames.set(key, null)
		}
	}
	return displayNames.get(key) ?? null
}

/** The language's name in `locale` (`Spanish`, `español`), or null when unknown */
export function languageName(code: string, locale?: string): string | null {
	try {
		const name = namesFor(locale)?.of(code)
		if (name && name.toLowerCase() !== code.toLowerCase()) {
			return name.charAt(0).toLocaleUpperCase(locale) + name.slice(1)
		}
	} catch {
		// not a language code Intl knows
	}
	return null
}

export type BookLanguage = {
	/** What the chip shows: `ES` */
	label: string
	/** Its name for the tooltip: `Spanish`, or the code when the name is unknown */
	name: string
}

/** What a book's language chip shows, or null when the book has no (readable) language */
export function bookLanguage(value?: string | null, locale?: string): BookLanguage | null {
	const code = bookLanguageCode(value)
	if (!code) return null
	return { label: code.toUpperCase(), name: languageName(code, locale) ?? code.toUpperCase() }
}
