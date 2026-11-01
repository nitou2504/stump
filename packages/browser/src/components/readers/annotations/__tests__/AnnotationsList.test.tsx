import { AnnotationStyle } from '@stump/graphql'
import { fireEvent, render, screen } from '@testing-library/react'

import AnnotationsList from '../AnnotationsList'
import type { ReaderAnnotation } from '../types'

vi.mock('@stump/i18n', () => ({
	...vi.importActual('@stump/i18n'),
	useCreateTranslate: () => ({
		t: (s: string) => s,
		translate: (s: string, options?: Record<string, unknown>) =>
			options?.page != null ? `${s}:${options.page}` : s,
	}),
}))

function annotation(overrides: Partial<ReaderAnnotation>): ReaderAnnotation {
	return {
		id: 'a',
		mediaId: 'media-1',
		userId: 'user-1',
		annotationText: null,
		style: AnnotationStyle.Highlight,
		createdAt: '2026-01-01T00:00:00Z',
		updatedAt: '2026-01-01T00:00:00Z',
		locator: { href: 'chapter.xhtml', type: 'application/xhtml+xml', chapterTitle: 'One' },
		...overrides,
	}
}

describe('AnnotationsList', () => {
	it('lists annotations in reading order with their quote and note', () => {
		const onSelect = vi.fn()
		render(
			<AnnotationsList
				onSelect={onSelect}
				annotations={[
					annotation({
						id: 'later',
						locator: {
							href: 'c2.xhtml',
							type: 'application/xhtml+xml',
							chapterTitle: 'Two',
							locations: { totalProgression: 0.8 },
							text: { highlight: 'second quote' },
						},
					}),
					annotation({
						id: 'earlier',
						style: AnnotationStyle.Strikethrough,
						annotationText: 'a note',
						locator: {
							href: 'c1.xhtml',
							type: 'application/xhtml+xml',
							chapterTitle: 'One',
							locations: { totalProgression: 0.2 },
							text: { highlight: 'first quote' },
						},
					}),
				]}
			/>,
		)

		const rows = screen.getAllByRole('button')
		expect(rows.map((row) => row.getAttribute('data-annotation-id'))).toEqual(['earlier', 'later'])
		expect(screen.getByText('first quote')).toHaveClass('line-through')
		expect(screen.getByText('a note')).toBeInTheDocument()

		fireEvent.click(rows[1]!)
		expect(onSelect).toHaveBeenCalledWith(expect.objectContaining({ id: 'later' }))
	})

	it('names the page of a PDF annotation', () => {
		render(
			<AnnotationsList
				onSelect={vi.fn()}
				annotations={[
					annotation({
						locator: {
							href: 'book.pdf',
							type: 'application/pdf',
							locations: { position: 12, rects: [{ x: 0.1, y: 0.5, width: 0.2, height: 0.02 }] },
							text: { highlight: 'quote' },
						},
					}),
				]}
			/>,
		)
		expect(screen.getByText('shared.readerAnnotations.page:12')).toBeInTheDocument()
	})
})
