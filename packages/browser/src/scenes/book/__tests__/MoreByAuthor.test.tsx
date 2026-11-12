import { render, screen } from '@testing-library/react'

import MoreByAuthor from '../MoreByAuthor'

vi.mock('@/hooks/useTranslate', () => ({
	useTranslate: () => ({
		t: (key: string, options?: Record<string, string>) =>
			options ? `${key} ${JSON.stringify(options)}` : key,
		translate: (key: string) => key,
	}),
}))
vi.mock('@stump/client', async (importOriginal) => ({
	...(await importOriginal<typeof import('@stump/client')>()),
	useGraphQL: () => ({ data: { media: { nodes: [{ id: 'other', seriesId: null }] } } }),
}))
vi.mock('@/components/book/BookCard', () => ({ default: () => null }))
vi.mock('@/components/HorizontalCardList', () => ({
	default: ({ title }: { title: string }) => <h2>{title}</h2>,
}))

describe('MoreByAuthor', () => {
	it('titles the row through the translations, with the author in it (not "More by" in English)', () => {
		render(<MoreByAuthor bookId="book" author="Frank Herbert" />)
		expect(
			screen.getByRole('heading', {
				name: 'bookOverviewScene.moreBy {"author":"Frank Herbert"}',
			}),
		).toBeInTheDocument()
	})
})
