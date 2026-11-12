import '@/__mocks__/resizeObserver'

import { Form } from '@stump/components'
import { UserPermission } from '@stump/graphql'
import { render, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useForm } from 'react-hook-form'

import { CreateOrUpdateUserSchema, formDefaults } from '../schema'
import UserPermissionsTable from '../UserPermissionsTable'

vi.mock('@/hooks/useTranslate', () => ({
	useTranslate: () => ({ t: (key: string) => key }),
}))

vi.mock('@/paths', () => ({
	usePaths: () => ({ docs: () => 'https://example.test/docs' }),
}))

/** What the form holds, read back by a test */
let held: UserPermission[] = []

const Subject = () => {
	const form = useForm<Pick<CreateOrUpdateUserSchema, 'permissions'>>({
		defaultValues: formDefaults(),
	})
	held = form.watch('permissions')

	return (
		<Form form={form} onSubmit={vi.fn()}>
			<UserPermissionsTable />
		</Form>
	)
}

const row = (container: HTMLElement, permission: UserPermission) =>
	container.querySelector(`[id="${permission}"]`)

describe('UserPermissionsTable', () => {
	it('offers the permissions that are in use', () => {
		const { container } = render(<Subject />)

		// a control, so that the absence below is the table's and not an empty render
		expect(row(container, UserPermission.AccessSmartList)).not.toBeNull()
		expect(row(container, UserPermission.AddBooks)).not.toBeNull()
		expect(row(container, UserPermission.DownloadFile)).not.toBeNull()
	})

	it('has no row for the book clubs, which are off', () => {
		const { container } = render(<Subject />)

		expect(row(container, UserPermission.AccessBookClub)).toBeNull()
		expect(row(container, UserPermission.CreateBookClub)).toBeNull()
		expect(container.textContent).not.toContain(UserPermission.AccessBookClub)
		expect(container.textContent).not.toContain(UserPermission.CreateBookClub)
	})

	// the table used to tick Scan library with Upload files, which made every friend who could add a
	// file hold the permission that starts a scan of a whole library. The server never implied it
	it('ticks nothing else when Upload files is ticked', async () => {
		const { container } = render(<Subject />)
		const user = userEvent.setup()
		held = []

		await user.click(row(container, UserPermission.UploadFile)!)

		await waitFor(() => expect(held).toContain(UserPermission.UploadFile))
		expect(held).toEqual([UserPermission.UploadFile])
		expect(held).not.toContain(UserPermission.ScanLibrary)
	})

	it('still ticks what a permission really implies', async () => {
		const { container } = render(<Subject />)
		const user = userEvent.setup()

		await user.click(row(container, UserPermission.ManageLibrary)!)

		await waitFor(() => expect(held).toContain(UserPermission.ScanLibrary))
		expect(held).toEqual(
			expect.arrayContaining([
				UserPermission.ManageLibrary,
				UserPermission.ScanLibrary,
				UserPermission.EditLibrary,
				UserPermission.EditThumbnails,
			]),
		)
	})
})
