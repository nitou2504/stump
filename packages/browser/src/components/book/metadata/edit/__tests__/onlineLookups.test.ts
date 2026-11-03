import { IntakeSource, OnlineLookupStatus } from '@stump/graphql'
import { describe, expect, it } from 'vitest'

import { lookupLine, lookupNeedsAttention } from '../onlineLookups'

describe('online lookups', () => {
	it('names each source and says how it answered', () => {
		expect(
			lookupLine({
				source: IntakeSource.GoogleBooks,
				status: OnlineLookupStatus.RateLimited,
				subjects: 0,
				message:
					'daily limit reached: add a Google Books API key in Settings → Metadata Integrations',
			}),
		).toBe(
			'Google Books: daily limit reached: add a Google Books API key in Settings → Metadata Integrations',
		)
		expect(
			lookupLine({
				source: IntakeSource.OpenLibrary,
				status: OnlineLookupStatus.NoSubjects,
				subjects: 0,
				message: 'found, but it has no subjects for this book',
			}),
		).toBe('Open Library: found, but it has no subjects for this book')
	})

	it('flags failures, not empty answers', () => {
		expect(lookupNeedsAttention(OnlineLookupStatus.RateLimited)).toBe(true)
		expect(lookupNeedsAttention(OnlineLookupStatus.Error)).toBe(true)
		expect(lookupNeedsAttention(OnlineLookupStatus.NoSubjects)).toBe(false)
		expect(lookupNeedsAttention(OnlineLookupStatus.NoMatch)).toBe(false)
		expect(lookupNeedsAttention(OnlineLookupStatus.Ok)).toBe(false)
	})
})
