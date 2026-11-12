import type { AllowedLocale } from '@stump/i18n'

/**
 * The language of somebody nobody knows yet (the sign-in and join screens, before there is a
 * person with a saved language): Spanish when the browser's first language is Spanish, else
 * English. The server's own pages decide the same way from `Accept-Language`, so the screen and
 * its error messages agree
 */
export function browserLocale(
	languages: readonly string[] | undefined = typeof navigator === 'undefined'
		? undefined
		: navigator.languages?.length
			? navigator.languages
			: [navigator.language],
): AllowedLocale {
	const first = languages?.[0]?.trim().toLowerCase() ?? ''
	return first === 'es' || first.startsWith('es-') ? 'es-ES' : 'en-US'
}
