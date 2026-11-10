import '@/__mocks__/resizeObserver'

import { Form } from '@stump/components'
import { UserPermission } from '@stump/graphql'
import { render } from '@testing-library/react'
import { useForm } from 'react-hook-form'

import { CreateOrUpdateUserSchema, formDefaults } from '../schema'
import UserPermissionsTable from '../UserPermissionsTable'

vi.mock('@/hooks/useTranslate', () => ({
	useTranslate: () => ({ t: (key: string) => key }),
}))

vi.mock('@/paths', () => ({
	usePaths: () => ({ docs: () => 'https://example.test/docs' }),
}))

const Subject = () => {
	const form = useForm<Pick<CreateOrUpdateUserSchema, 'permissions'>>({
		defaultValues: formDefaults(),
	})

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
})
