import { LocaleContext, type LocaleContextProps } from '@stump/i18n'
import i18next from 'i18next'
import { PropsWithChildren } from 'react'

import enUS from '../../../i18n/src/locales/en-US.json'
import esES from '../../../i18n/src/locales/es-ES.json'

const translations = { 'en-US': enUS, 'es-ES': esES }

/**
 * The app's real translations for a test that reads what is shown: the same files, the same
 * plurals and filling in (a key a language lacks falls back to English, as it does in the app).
 * Use it as a wrapper: `render(<Page />, { wrapper: localeWrapper('es-ES') })`
 */
export function localeWrapper(locale: keyof typeof translations) {
	const i18n = i18next.createInstance()
	void i18n.init({
		lng: locale,
		fallbackLng: 'en-US',
		defaultNS: locale,
		fallbackNS: 'en-US',
		ns: [locale, 'en-US'],
		resources: { 'en-US': { 'en-US': enUS }, [locale]: { [locale]: translations[locale] } },
		interpolation: { escapeValue: false },
		initAsync: false,
	})

	const t = i18n.t.bind(i18n) as unknown as LocaleContextProps['t']

	return function LocaleWrapper({ children }: PropsWithChildren) {
		return <LocaleContext.Provider value={{ locale, t }}>{children}</LocaleContext.Provider>
	}
}
