import { formatBytes } from '@stump/client'

import { bookCardDetail } from '../bookCardDetail'

describe('bookCardDetail', () => {
	it('says who wrote the book', () => {
		expect(bookCardDetail(['Frank Herbert'], 3_000_000)).toEqual({
			text: 'Frank Herbert',
			title: 'Frank Herbert',
		})
	})

	it('lists the authors in order, once each, so the card can cut the line where it ends', () => {
		expect(bookCardDetail(['Terry Pratchett', ' Neil Gaiman ', 'Terry Pratchett', ''], 1)).toEqual({
			text: 'Terry Pratchett, Neil Gaiman',
			title: 'Terry Pratchett, Neil Gaiman',
		})
	})

	it.each([[undefined], [null], [[]], [['', '  ', null]]])(
		'falls back to the file size when no author is named (%j)',
		(writers) => {
			const detail = bookCardDetail(writers as never, 376_000)
			expect(detail).toEqual({ text: formatBytes(376_000) ?? '' })
			expect(detail.text).not.toBe('')
		},
	)
})
