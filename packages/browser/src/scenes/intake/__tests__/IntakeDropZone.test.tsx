import { IntakeUploadMode } from '@stump/graphql'
import { fireEvent, render, screen, within } from '@testing-library/react'

import type { UploadEntry } from '../hooks'
import IntakeDropZone from '../IntakeDropZone'

const entry = (key: string, overrides: Partial<UploadEntry> = {}): UploadEntry => ({
	files: [],
	isFolder: false,
	key,
	label: `${key}.epub`,
	mode: IntakeUploadMode.Auto,
	progress: 0,
	status: 'queued',
	totalBytes: 2 * 1024 * 1024,
	...overrides,
})

const libraries = [
	{ id: 'lib', name: 'Biblioteca' },
	{ id: 'manga', name: 'Manga' },
]

function renderZone(props: Partial<Parameters<typeof IntakeDropZone>[0]> = {}) {
	const onFiles = vi.fn()
	const onClearFinished = vi.fn()
	const onLibraryChange = vi.fn()
	const view = render(
		<IntakeDropZone
			onFiles={onFiles}
			entries={[]}
			onClearFinished={onClearFinished}
			onLibraryChange={onLibraryChange}
			{...props}
		/>,
	)
	return { ...view, onClearFinished, onFiles, onLibraryChange }
}

/** The input that picks a folder (the other one is react-dropzone's own) */
const folderInput = (container: HTMLElement) =>
	container.querySelector('input[webkitdirectory]') as HTMLInputElement

describe('IntakeDropZone', () => {
	it('is a big, friendly target while the inbox is empty', () => {
		renderZone()

		expect(screen.getByText('Drop books here')).toBeInTheDocument()
		expect(screen.getByText(/EPUB, PDF, CBZ, CBR or a zip; folders work too/)).toBeInTheDocument()
		expect(screen.getByRole('button', { name: /Choose files/ })).toBeInTheDocument()
		expect(screen.getByRole('button', { name: /Choose a folder/ })).toBeInTheDocument()
	})

	it('shrinks to a bar when the inbox has books, with the same two buttons', () => {
		renderZone({ compact: true })

		expect(screen.queryByText('Drop books here')).not.toBeInTheDocument()
		expect(screen.getByText('Drop more books here')).toBeInTheDocument()
		expect(screen.getByRole('button', { name: /Choose files/ })).toBeInTheDocument()
		expect(screen.getByRole('button', { name: /Choose a folder/ })).toBeInTheDocument()
	})

	it('keeps the keyboard focus on “Choose files” when the zone shrinks to a bar', () => {
		const { onClearFinished, onFiles, rerender } = renderZone()
		const choose = screen.getByRole('button', { name: /Choose files/ })
		choose.focus()
		expect(choose).toHaveFocus()

		rerender(
			<IntakeDropZone onFiles={onFiles} entries={[]} onClearFinished={onClearFinished} compact />,
		)

		// the same button, not a copy of it: a copy would drop the focus to the page
		expect(screen.getByRole('button', { name: /Choose files/ })).toBe(choose)
		expect(choose).toHaveFocus()
	})

	it('is not a tab stop of its own, big or small: the two buttons are the way in', () => {
		const { container, onClearFinished, onFiles, rerender } = renderZone()
		expect(container.querySelector('[tabindex="0"]')).toBeNull()

		rerender(
			<IntakeDropZone onFiles={onFiles} entries={[]} onClearFinished={onClearFinished} compact />,
		)

		expect(container.querySelector('[tabindex="0"]')).toBeNull()
	})

	it('does not offer zips to somebody whose books are sent for approval', () => {
		renderZone({ acceptsZip: false })

		expect(screen.getByText(/EPUB, PDF, CBZ or CBR; folders work too/)).toBeInTheDocument()
		expect(screen.queryByText(/zip/i)).not.toBeInTheDocument()
	})

	it('shrinks while uploads are on their way, even in an empty inbox', () => {
		renderZone({ entries: [entry('a')] })

		expect(screen.getByText('Drop more books here')).toBeInTheDocument()
	})

	it('picks the library the books go to, when there are several', () => {
		const { onLibraryChange } = renderZone({ libraries, libraryId: 'lib' })

		const select = screen.getByLabelText('Add to')
		expect(select).toHaveValue('lib')
		fireEvent.change(select, { target: { value: 'manga' } })

		expect(onLibraryChange).toHaveBeenCalledWith('manga')
	})

	it('has no library to pick when there is only one', () => {
		renderZone({ libraries: [libraries[0] as { id: string; name: string }], libraryId: 'lib' })

		expect(screen.queryByLabelText('Add to')).not.toBeInTheDocument()
	})

	it('reads a folder as one series, unless it is told they are separate books', () => {
		const { container, onFiles } = renderZone()
		const file = new File(['x'], 'v1.cbz')

		fireEvent.change(folderInput(container), { target: { files: [file] } })
		expect(onFiles).toHaveBeenLastCalledWith(
			[{ file, relativePath: undefined }],
			IntakeUploadMode.Auto,
		)

		fireEvent.change(screen.getByLabelText('Each folder is'), {
			target: { value: IntakeUploadMode.Books },
		})
		fireEvent.change(folderInput(container), { target: { files: [file] } })
		expect(onFiles).toHaveBeenLastCalledWith(
			[{ file, relativePath: undefined }],
			IntakeUploadMode.Books,
		)
	})

	it('cannot be used while uploads are closed', () => {
		renderZone({ disabled: true, libraries, libraryId: 'lib' })

		expect(screen.getByRole('button', { name: /Choose files/ })).toBeDisabled()
		expect(screen.getByRole('button', { name: /Choose a folder/ })).toBeDisabled()
		expect(screen.getByLabelText('Add to')).toBeDisabled()
		expect(screen.getByLabelText('Each folder is')).toBeDisabled()
	})
})

describe('IntakeDropZone uploads', () => {
	it('shows no list until there is something to upload', () => {
		renderZone()

		expect(screen.queryByText(/Uploads ·/)).not.toBeInTheDocument()
	})

	it('shows an upload with a bar and its percentage while it goes', () => {
		renderZone({ entries: [entry('a', { progress: 40, status: 'uploading' })] })

		expect(screen.getByRole('progressbar', { name: 'Uploading a.epub' })).toBeInTheDocument()
		expect(screen.getByText('40%')).toBeInTheDocument()
		expect(screen.getByLabelText('Uploading')).toBeInTheDocument()
	})

	it('shows what waits, with why when the server asked to slow down', () => {
		renderZone({
			entries: [entry('a', { note: 'The server asked to slow down: trying again in 6 s' })],
		})

		expect(screen.getByLabelText('Waiting')).toBeInTheDocument()
		expect(screen.getByText(/trying again in 6 s/)).toBeInTheDocument()
		// what waits has its size, not a percentage
		expect(screen.getByText('2.0 MB')).toBeInTheDocument()
	})

	it('says why an upload failed, in the words of the server', () => {
		renderZone({
			entries: [entry('a', { error: 'That file is bigger than 100 MB', status: 'error' })],
		})

		expect(screen.getByLabelText('Failed')).toBeInTheDocument()
		expect(screen.getByText('That file is bigger than 100 MB')).toBeInTheDocument()
	})

	it('names a folder with the number of files in it', () => {
		renderZone({
			entries: [
				entry('Yotsuba', {
					files: [new File(['x'], 'v1.cbz'), new File(['x'], 'v2.cbz')],
					isFolder: true,
					label: 'Yotsuba',
				}),
			],
		})

		expect(screen.getByText(/· 2 files/)).toBeInTheDocument()
	})

	it('counts what is done, and clears it', () => {
		const { onClearFinished } = renderZone({
			entries: [
				entry('a', { status: 'done' }),
				entry('b', { status: 'done' }),
				entry('c', { progress: 10, status: 'uploading' }),
			],
		})

		expect(screen.getByText('Uploads · 2 of 3 done')).toBeInTheDocument()
		expect(screen.getAllByLabelText('Uploaded')).toHaveLength(2)
		fireEvent.click(screen.getByRole('button', { name: 'Clear finished' }))

		expect(onClearFinished).toHaveBeenCalledTimes(1)
	})

	it('has nothing to clear until an upload is done', () => {
		renderZone({ entries: [entry('a', { progress: 10, status: 'uploading' })] })

		expect(screen.queryByRole('button', { name: 'Clear finished' })).not.toBeInTheDocument()
		expect(
			within(screen.getByRole('region', { name: 'Upload books' })).getByText(
				'Uploads · 0 of 1 done',
			),
		).toBeInTheDocument()
	})
})
