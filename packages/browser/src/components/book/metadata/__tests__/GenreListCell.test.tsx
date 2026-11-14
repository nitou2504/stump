import { render, screen } from '@testing-library/react'
import type { ReactNode } from 'react'
import { FormProvider, useForm } from 'react-hook-form'
import { MemoryRouter } from 'react-router'

import GenreListCell from '../GenreListCell'

vi.mock('@/components/metadata/metadataEditor/context', () => ({
	useMetadataEditorContext: () => ({ isEditing: false, isFieldLocked: () => false }),
}))
vi.mock('../edit/ValueChipsField', () => ({ default: () => null }))

function Forms({ children }: { children: ReactNode }) {
	const form = useForm()
	return <FormProvider {...form}>{children}</FormProvider>
}

describe('GenreListCell', () => {
	it('links every value and shows no empty chip for a blank one', () => {
		render(
			<MemoryRouter>
				<Forms>
					<GenreListCell
						field="genres"
						values={['', 'History', '   ', 'Level: General Reader']}
						itemUrl={(item) => `/books?genre=${encodeURIComponent(item)}`}
					/>
				</Forms>
			</MemoryRouter>,
		)

		const links = screen.getAllByRole('link')
		expect(links.map((link) => link.textContent)).toEqual(['History', 'Level: General Reader'])
		expect(links.map((link) => link.getAttribute('href'))).toEqual([
			'/books?genre=History',
			'/books?genre=Level%3A%20General%20Reader',
		])
	})
})
