import { isMangaBook } from '../useBookPreferences'

describe('isMangaBook', () => {
	it('recognises manga by its genre (ComicInfo Genre or the curator tags)', () => {
		expect(isMangaBook({ metadata: { genres: ['Manga & Comics', 'Manga & Comics: Manga'] } })).toBe(
			true,
		)
		expect(isMangaBook({ metadata: { genres: ['manga'] } })).toBe(true)
	})

	it("doesn't take western comics or a missing genre for manga", () => {
		expect(
			isMangaBook({ metadata: { genres: ['Manga & Comics', 'Manga & Comics: Comics'] } }),
		).toBe(false)
		expect(isMangaBook({ metadata: { genres: [] } })).toBe(false)
		expect(isMangaBook({ metadata: null })).toBe(false)
		expect(isMangaBook({})).toBe(false)
	})
})
