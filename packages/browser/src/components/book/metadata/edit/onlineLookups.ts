import { IntakeSource, OnlineLookupStatus } from '@stump/graphql'

type Lookup = {
	source: IntakeSource
	status: OnlineLookupStatus
	subjects: number
	message: string
}

const SOURCE_NAME: Partial<Record<IntakeSource, string>> = {
	[IntakeSource.OpenLibrary]: 'Open Library',
	[IntakeSource.GoogleBooks]: 'Google Books',
	[IntakeSource.Hardcover]: 'Hardcover',
}

/** `Hardcover`, `Open Library`, `Google Books` */
export const sourceName = (source: IntakeSource): string => SOURCE_NAME[source] ?? 'Online'

/** "Open Library: found, 3 subjects", "Google Books: daily limit reached: add …" */
export function lookupLine(lookup: Lookup): string {
	return `${sourceName(lookup.source)}: ${lookup.message}`
}

/** A lookup that failed (the source refused or errored), as opposed to one that found nothing */
export const lookupNeedsAttention = (status: OnlineLookupStatus) =>
	status === OnlineLookupStatus.RateLimited || status === OnlineLookupStatus.Error
