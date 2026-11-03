import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

import { EmptyConfirmation, PurgeConfirmation } from '../Confirmations'
import type { DeletedBookEntry } from '../DeletedBookRow'
import RetentionFuse from '../RetentionFuse'

const entry = (id: string, title: string, fileExists = true): DeletedBookEntry => ({
	deletedAt: '2026-11-03T16:06:19Z',
	deletedBy: 'g3',
	expiresAt: '2026-11-17T16:06:19Z',
	purgeAt: '2026-11-18T04:00:00Z',
	libraryId: 'lib',
	libraryName: 'Biblioteca',
	relativePath: `Author/${title}.epub`,
	fileExists,
	book: {
		id,
		resolvedName: title,
		extension: 'epub',
		size: 1000,
		thumbnail: { url: `/api/v2/media/${id}/thumbnail` },
		metadata: { writers: ['Author'] },
	},
})

describe('PurgeConfirmation', () => {
	it('says the file is deleted and what Stump erases', () => {
		render(
			<PurgeConfirmation
				entries={[entry('a', 'El nombre de la rosa')]}
				isPending={false}
				onConfirm={vi.fn()}
				onClose={vi.fn()}
			/>,
		)
		expect(screen.getByText('Delete “El nombre de la rosa” permanently?')).toBeTruthy()
		expect(screen.getByText(/Its file is deleted from disk/)).toBeTruthy()
		expect(
			screen.getByText(/erases its reading progress, annotations, bookmarks and tags/),
		).toBeTruthy()
		expect(screen.getByRole('button', { name: 'Delete permanently' })).toBeTruthy()
	})

	it('counts the files deleted and the ones already gone', () => {
		render(
			<PurgeConfirmation
				entries={[entry('a', 'On disk'), entry('b', 'Gone', false)]}
				isPending={false}
				onConfirm={vi.fn()}
				onClose={vi.fn()}
			/>,
		)
		expect(screen.getByText('Delete 2 books permanently?')).toBeTruthy()
		expect(screen.getByText(/1 file is deleted from disk/)).toBeTruthy()
		expect(screen.getByText(/1 file is already gone from the library/)).toBeTruthy()
		expect(screen.queryByText(/quarantine/)).toBeNull()
	})
})

describe('EmptyConfirmation', () => {
	it('empties only once the phrase is typed', () => {
		const onConfirm = vi.fn()
		render(
			<EmptyConfirmation
				isOpen
				entries={[entry('a', 'One'), entry('b', 'Two'), entry('c', 'Three')]}
				isPending={false}
				onConfirm={onConfirm}
				onClose={vi.fn()}
			/>,
		)
		const confirm = screen.getByRole('button', { name: 'Empty Deleted books' })
		expect((confirm as HTMLButtonElement).disabled).toBe(true)
		fireEvent.change(screen.getByPlaceholderText('delete 3 books'), {
			target: { value: 'delete 2 books' },
		})
		expect((confirm as HTMLButtonElement).disabled).toBe(true)
		fireEvent.change(screen.getByPlaceholderText('delete 3 books'), {
			target: { value: 'Delete 3 books' },
		})
		expect((confirm as HTMLButtonElement).disabled).toBe(false)
		fireEvent.click(confirm)
		expect(onConfirm).toHaveBeenCalledTimes(1)
	})
})

describe('RetentionFuse', () => {
	it('draws the days left and says when the book goes', () => {
		const { container } = render(
			<RetentionFuse
				purgeAt={new Date(2026, 10, 15, 23, 0).toISOString()}
				retentionDays={14}
				now={new Date(2026, 10, 3, 12, 0)}
			/>,
		)
		expect(screen.getByText('Deleted permanently in 12 days')).toBeTruthy()
		const ticks = container.querySelectorAll(
			'[data-testid="retention-fuse"] > span[aria-hidden] > span',
		)
		expect(ticks).toHaveLength(14)
	})

	it('says when books are kept until deleted by hand', () => {
		render(<RetentionFuse purgeAt={null} retentionDays={0} now={new Date()} />)
		expect(screen.getByText('Kept until deleted by hand')).toBeTruthy()
	})
})
