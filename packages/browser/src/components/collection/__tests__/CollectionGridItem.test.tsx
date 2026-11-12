import { render, screen } from '@testing-library/react'
import { Book } from 'lucide-react'
import { MemoryRouter } from 'react-router'

import { CollectionGridItem } from '../CollectionGridItem'

vi.mock('../CollectionStackedThumbnails', () => ({ CollectionStackedThumbnails: () => null }))

function renderCard(stats: Parameters<typeof CollectionGridItem>[0]['stats']) {
	return render(
		<MemoryRouter>
			<CollectionGridItem
				title="Recs"
				to="/recs"
				thumbnails={[]}
				layoutNumber={undefined}
				stats={stats}
			/>
		</MemoryRouter>,
	)
}

describe('CollectionGridItem chips', () => {
	it('says what a count counts: the word follows the number on the chip', () => {
		renderCard([{ key: 'books', icon: Book, value: '344', unit: 'books', label: '344 books' }])

		const chip = screen.getByTitle('344 books')
		expect(chip).toHaveTextContent('344 books')
	})

	it('shows a chip with no word as it is (who made a list, for one)', () => {
		renderCard([{ key: 'by', icon: Book, value: 'by Ana', label: 'Made by Ana' }])

		expect(screen.getByTitle('Made by Ana')).toHaveTextContent(/^by Ana$/)
	})
})
