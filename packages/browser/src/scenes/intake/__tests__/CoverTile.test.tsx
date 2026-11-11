import { fireEvent, render, screen, waitFor } from '@testing-library/react'

import CoverTile, { BookCover, hueOf, monogramOf } from '../CoverTile'

const sdk = vi.hoisted(() => ({
	isTokenAuth: false,
	media: { thumbnailURL: (id: string) => `/thumbnail/${id}` },
}))
vi.mock('@stump/client', async (importOriginal) => ({
	...(await importOriginal<typeof import('@stump/client')>()),
	useSDK: () => ({ sdk }),
}))

describe('monogramOf', () => {
	it('takes the first letters of the first two words', () => {
		expect(monogramOf('El nombre de la rosa')).toBe('EN')
		expect(monogramOf('Dune')).toBe('D')
	})

	it('skips punctuation, and reads any alphabet', () => {
		expect(monogramOf('“¡Hola, mundo!”')).toBe('HM')
		expect(monogramOf('進撃の巨人')).toBe('進')
		expect(monogramOf('Éloge de la lenteur')).toBe('ÉD')
	})

	it('has nothing for a title with no letters', () => {
		expect(monogramOf('')).toBe('')
		expect(monogramOf('— …')).toBe('')
	})
})

describe('hueOf', () => {
	it('gives a book the same colour every time, within the colour wheel', () => {
		expect(hueOf('Dune')).toBe(hueOf('Dune'))
		for (const title of ['Dune', 'Emma', 'El nombre de la rosa', '', '進撃の巨人']) {
			expect(hueOf(title)).toBeGreaterThanOrEqual(0)
			expect(hueOf(title)).toBeLessThan(360)
		}
	})

	it('tells different books apart', () => {
		const hues = new Set(['Dune', 'Emma', 'Ulysses', 'Hamlet', 'Walden'].map(hueOf))
		expect(hues.size).toBeGreaterThan(1)
	})
})

describe('CoverTile', () => {
	it('is decoration: the title next to it already says which book it is', () => {
		const { container } = render(<CoverTile title="Dune" format="epub" />)

		expect(container.firstElementChild).toHaveAttribute('aria-hidden', 'true')
	})

	it('shows the monogram on the card size, and only the format on the row size', () => {
		const { rerender } = render(<CoverTile title="El nombre de la rosa" format="epub" />)
		expect(screen.getByText('EN')).toBeInTheDocument()

		rerender(<CoverTile title="El nombre de la rosa" format="epub" size="sm" />)
		expect(screen.queryByText('EN')).not.toBeInTheDocument()
	})

	it('is tinted from the title', () => {
		const { container } = render(<CoverTile title="Dune" />)

		const tile = container.firstElementChild as HTMLElement
		expect(tile.style.backgroundColor).not.toBe('')
		expect(tile.style.borderColor).not.toBe('')
	})
})

describe('BookCover', () => {
	it('shows the thumbnail of the book the scan made, once it has loaded', () => {
		const { container } = render(<BookCover mediaId="m1" title="Dune" />)

		const image = container.querySelector('img[src="/thumbnail/m1"]') as HTMLImageElement
		expect(image).not.toBeNull()
		// the tile is there until the picture is
		expect(image).toHaveClass('opacity-0')
		fireEvent.load(image)
		expect(image).toHaveClass('opacity-100')
	})

	it('keeps the tile under the picture, for the time it takes to load', () => {
		render(<BookCover mediaId="m1" title="Dune" size="md" />)

		expect(screen.getByText('D')).toBeInTheDocument()
	})

	it('falls back to the tile when the picture can not be had', async () => {
		const { container } = render(<BookCover mediaId="m1" title="Dune" size="md" />)

		fireEvent.error(container.querySelector('img') as HTMLImageElement)

		await waitFor(() => expect(container.querySelector('img')).toBeNull())
		expect(screen.getByText('D')).toBeInTheDocument()
	})
})
