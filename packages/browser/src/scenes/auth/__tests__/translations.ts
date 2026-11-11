/**
 * The words of the screens are their keys (with the values asked for after them), so a test reads
 * what a screen says by its key. A module of its own, so that `vi.mock` can load it
 */
export const useTranslate = () => ({
	t: (key: string, options?: Record<string, unknown>) =>
		options && Object.keys(options).length
			? `${key} ${Object.entries(options)
					.map(([name, value]) => `${name}=${String(value)}`)
					.join(' ')}`
			: key,
	translate: (key: string) => key,
})
