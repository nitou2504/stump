import { IntakeSeriesKind, LibraryType } from '@stump/graphql'
import { fireEvent, render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router'

import IntakeItemCard from '../IntakeItemCard'
import type { IntakeItemData } from '../queries'
import { item } from './fixtures'

// the pickers ask the server for folders and tags: not what is tested here
vi.mock('../FolderPicker', () => ({ default: () => <div>Folder picker</div> }))
vi.mock('../TagPicker', () => ({ default: () => <div>Tag picker</div> }))

const biblioteca = { id: 'lib', name: 'Biblioteca' }
const libraries = [biblioteca, { id: 'manga', name: 'Manga' }]

function renderCard(
	overrides: Partial<IntakeItemData> = {},
	props: Partial<Parameters<typeof IntakeItemCard>[0]> = {},
) {
	const onChangeLibrary = vi.fn()
	render(
		<MemoryRouter>
			<IntakeItemCard
				item={item(overrides)}
				libraries={libraries}
				onChangeLibrary={onChangeLibrary}
				onReview={vi.fn()}
				onAccept={vi.fn()}
				onDiscard={vi.fn()}
				onReanalyse={vi.fn()}
				{...props}
			/>
		</MemoryRouter>,
	)
	return { onChangeLibrary }
}

describe('IntakeItemCard library', () => {
	it('moves the item to the library picked in the switch', () => {
		const { onChangeLibrary } = renderCard()

		const select = screen.getByLabelText('Library')
		expect(select).toHaveValue('lib')
		fireEvent.change(select, { target: { value: 'manga' } })

		expect(onChangeLibrary).toHaveBeenCalledWith('a', 'manga')
	})

	it('does not move the item when the library it is in is picked again', () => {
		const { onChangeLibrary } = renderCard()

		fireEvent.change(screen.getByLabelText('Library'), { target: { value: 'lib' } })

		expect(onChangeLibrary).not.toHaveBeenCalled()
	})

	it('has no switch, and names the library, when there is nowhere else to go', () => {
		renderCard({}, { libraries: [biblioteca] })

		expect(screen.queryByLabelText('Library')).not.toBeInTheDocument()
		expect(screen.getByText(/rosa\.epub · Biblioteca/)).toBeInTheDocument()
	})

	it('offers the library of its own the server suggests, and moves the item to it', () => {
		const { onChangeLibrary } = renderCard({ suggestedLibrary: { id: 'manga', name: 'Manga' } })

		fireEvent.click(screen.getByRole('button', { name: 'Move to Manga' }))

		expect(onChangeLibrary).toHaveBeenCalledWith('a', 'manga')
	})

	it('offers nothing when the server suggests no library', () => {
		renderCard({ suggestedLibrary: null })

		expect(screen.queryByRole('button', { name: /^Move to/ })).not.toBeInTheDocument()
	})

	it('lets the uploader choose what a book is filed under in a book library', () => {
		renderCard({ libraryType: LibraryType.Book })

		expect(screen.getByRole('radiogroup', { name: 'File it under' })).toBeInTheDocument()
	})

	it('has nothing to choose in a manga or light novel library, which holds one kind of series', () => {
		const { decision } = item()
		renderCard({
			decision: decision ? { ...decision, seriesKind: IntakeSeriesKind.Manga } : null,
			libraryType: LibraryType.Manga,
		})

		expect(screen.queryByRole('radiogroup', { name: 'File it under' })).not.toBeInTheDocument()
		expect(screen.getByText('Manga series')).toBeInTheDocument()
	})
})
