import '@/__mocks__/resizeObserver'

import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import ReadingListFormDialog from '../ReadingListFormDialog'
import { sent, setup } from './support'

vi.mock('@/hooks/useTranslate', () => import('./translations'))

function renderDialog(props: Partial<React.ComponentProps<typeof ReadingListFormDialog>> = {}) {
	const { execute, Wrapper } = setup({
		answers: (query) => {
			if (query.includes('mutation CreateReadingList'))
				return { createReadingList: { id: 'new', name: 'Sci-fi' } }
			if (query.includes('mutation UpdateReadingList')) return { updateReadingList: { id: 'list' } }
			throw new Error(`Unexpected query ${query}`)
		},
	})
	const onClose = vi.fn()
	render(<ReadingListFormDialog isOpen onClose={onClose} {...props} />, { wrapper: Wrapper })
	return { execute, onClose }
}

const visibility = () => screen.getByRole('switch', { name: 'readingLists.form.public.label' })

describe('ReadingListFormDialog', () => {
	it('makes a private list unless the maker shows it to everyone', async () => {
		const user = userEvent.setup()
		const { execute, onClose } = renderDialog()

		// off by default, and says who would see the list
		expect(visibility()).toHaveAttribute('aria-checked', 'false')
		expect(screen.getByText('readingLists.form.public.hint')).toBeInTheDocument()

		await user.type(screen.getByRole('textbox', { name: /^readingLists\.form\.name/ }), ' Sci-fi ')
		await user.click(screen.getByRole('button', { name: 'shared.common.create' }))

		await waitFor(() =>
			expect(sent(execute, 'CreateReadingList')).toEqual([
				{ input: { name: 'Sci-fi', description: '', visibility: 'PRIVATE' } },
			]),
		)
		await waitFor(() => expect(onClose).toHaveBeenCalled())
	})

	it('makes a public list when the switch is on', async () => {
		const user = userEvent.setup()
		const { execute } = renderDialog()

		await user.type(screen.getByRole('textbox', { name: /^readingLists\.form\.name/ }), 'Recs')
		await user.click(visibility())
		expect(visibility()).toHaveAttribute('aria-checked', 'true')
		await user.click(screen.getByRole('button', { name: 'shared.common.create' }))

		await waitFor(() =>
			expect(sent(execute, 'CreateReadingList')).toEqual([
				{ input: { name: 'Recs', description: '', visibility: 'PUBLIC' } },
			]),
		)
	})

	it('shows how a list is shared when it is edited, and saves a change', async () => {
		const user = userEvent.setup()
		const { execute } = renderDialog({
			list: { id: 'list', name: 'Recs', description: 'From Ana', visibility: 'PUBLIC' },
		})

		expect(visibility()).toHaveAttribute('aria-checked', 'true')
		await user.click(visibility())
		await user.click(screen.getByRole('button', { name: 'shared.common.saveChanges' }))

		await waitFor(() =>
			expect(sent(execute, 'UpdateReadingList')).toEqual([
				{
					id: 'list',
					input: { name: 'Recs', description: 'From Ana', visibility: 'PRIVATE' },
				},
			]),
		)
	})

	it('shows a private list as private', () => {
		renderDialog({ list: { id: 'list', name: 'Secret', visibility: 'PRIVATE' } })
		expect(visibility()).toHaveAttribute('aria-checked', 'false')
	})
})
