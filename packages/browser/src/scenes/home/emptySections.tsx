import { createContext, useContext, useEffect } from 'react'

/** The home sections that hide themselves while the reader has nothing for them */
export type WatchedSection = 'continueReading' | 'onDeck'

type ReportEmpty = (section: WatchedSection, empty: boolean) => void

const HomeEmptySectionsContext = createContext<ReportEmpty | null>(null)

/** Where the home page hears from its sections: see {@link useReportEmptySection} */
export const HomeEmptySectionsProvider = HomeEmptySectionsContext.Provider

/**
 * A section that has nothing to show is not drawn (an empty box takes up the first screen of a
 * new reader's home), so it tells the page, which says so once for all of them when every one of
 * them is empty. Outside the home page this does nothing.
 */
export function useReportEmptySection(section: WatchedSection, empty: boolean) {
	const report = useContext(HomeEmptySectionsContext)

	useEffect(() => {
		report?.(section, empty)
		// a section that goes away is no longer an empty one
		return () => report?.(section, false)
	}, [report, section, empty])
}
