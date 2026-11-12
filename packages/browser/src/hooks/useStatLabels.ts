import { useTranslate } from './useTranslate'

/** What a count on a chip can be a count of */
export type StatWord = 'books' | 'series' | 'libraries' | 'lists' | 'authors' | 'genres'

/**
 * The words that go with the small number chips on headers and cards, so that no chip is a bare
 * number: "813 series", "8,756 books", "2 / 6 read".
 */
export function useStatLabels() {
	const { t, locale } = useTranslate()

	const number = (count: number) => count.toLocaleString(locale)
	const word = (kind: StatWord, count: number) => t(`statChips.${kind}`, { count })

	return {
		/** A count in the reader's number format: 8,756 */
		number,
		/** The word that follows a count, in the right plural: book / books */
		word,
		/** The whole phrase: "8,756 books" */
		count: (kind: StatWord, count: number) => `${number(count)} ${word(kind, count)}`,
		/** "read", as in "2 / 6 read" */
		read: () => t('statChips.read'),
	}
}
