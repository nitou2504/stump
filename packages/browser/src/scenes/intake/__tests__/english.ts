import en from '../../../../../i18n/src/locales/en-US.json'

/**
 * The words of the screens as the English locale has them, for the tests of screens whose words
 * are asserted as the person reads them (not as keys). A module of its own, so that `vi.mock` can
 * load it
 */
const lookup = (key: string): unknown =>
	key
		.split('.')
		.reduce<unknown>(
			(node, part) =>
				node && typeof node === 'object' ? (node as Record<string, unknown>)[part] : undefined,
			(en as unknown as { webApp: Record<string, unknown> }).webApp,
		)

export const translateEnglish = (key: string, options?: Record<string, unknown>): string => {
	const count = typeof options?.count === 'number' ? options.count : undefined
	let text = lookup(key)
	if (typeof text !== 'string' && count !== undefined) {
		text = lookup(`${key}_${count === 1 ? 'one' : 'other'}`)
	}
	if (typeof text !== 'string') return key
	return text.replace(/{{(\w+)}}/g, (_, name: string) => String(options?.[name] ?? ''))
}

export const useTranslate = () => ({ t: translateEnglish, translate: translateEnglish })
