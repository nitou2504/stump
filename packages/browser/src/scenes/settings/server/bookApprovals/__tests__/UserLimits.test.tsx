import { UserPermission } from '@stump/graphql'
import { fireEvent, render, screen, within } from '@testing-library/react'
import { MemoryRouter } from 'react-router'

import UserLimits, { parseLimit } from '../UserLimits'

vi.mock('@/hooks/useTranslate', () => import('@/scenes/intake/__tests__/english'))

const MB = 1024 * 1024

/** The section as the person's sheet in Public access opens it: with `?person=` the section is open on them */
const renderLimits = (path = '/settings/book-approvals?person=ana') =>
	render(
		<MemoryRouter initialEntries={[path]}>
			<UserLimits />
		</MemoryRouter>,
	)

const who = vi.hoisted(() => ({ canManageUsers: true }))
vi.mock('@/context', async (importOriginal) => ({
	...(await importOriginal<typeof import('@/context')>()),
	useAppContext: () => ({ checkPermission: () => who.canManageUsers }),
}))

const server = vi.hoisted(() => ({
	limits: {} as Record<string, unknown>,
	users: [] as unknown[],
}))
const mutate = vi.hoisted(() => vi.fn())
vi.mock('@stump/client', async (importOriginal) => ({
	...(await importOriginal<typeof import('@stump/client')>()),
	// the users query has no variables, the limits query asks for one person
	useGraphQL: (_document: unknown, _key: unknown, variables?: { userId: string }) =>
		variables
			? { data: { userIntakeLimits: server.limits[variables.userId] } }
			: { data: { users: { nodes: server.users } } },
	useGraphQLMutation: () => ({ isPending: false, mutate }),
}))
vi.mock('@tanstack/react-query', async (importOriginal) => ({
	...(await importOriginal<typeof import('@tanstack/react-query')>()),
	useQueryClient: () => ({ invalidateQueries: vi.fn() }),
}))

const effective = {
	maxFileBytes: 300 * MB,
	maxFilesPerRequest: 50,
	maxItems: 100,
	quotaBytes: 2048 * MB,
}
const person = (id: string, permissions: UserPermission[], isServerOwner = false) => ({
	id,
	isServerOwner,
	permissions,
	username: id,
})
const limits = (userId: string, own: Record<string, number | null> = {}) => ({
	effective,
	maxFileMb: null,
	maxFilesPerRequest: null,
	maxItems: null,
	quotaMb: null,
	userId,
	...own,
})

const boxes = () => ({
	files: screen.getByLabelText('Files per upload') as HTMLInputElement,
	items: screen.getByLabelText('Books at once') as HTMLInputElement,
	maxFile: screen.getByLabelText('Biggest file') as HTMLInputElement,
	quota: screen.getByLabelText('Inbox space') as HTMLInputElement,
})
const type = (box: HTMLInputElement, value: string) => fireEvent.change(box, { target: { value } })

describe('Upload limits', () => {
	beforeEach(() => {
		vi.clearAllMocks()
		who.canManageUsers = true
		server.users = [
			person('owner', [], true),
			person('ana', [UserPermission.AddBooks]),
			person('ben', [UserPermission.AddBooks]),
			person('carl', [UserPermission.ChangeAvatar]),
		]
		server.limits = {
			ana: limits('ana', { quotaMb: 500 }),
			ben: limits('ben'),
		}
	})

	it('is not there for somebody who cannot manage users: the server answers to nobody else', () => {
		who.canManageUsers = false

		const { container } = renderLimits()

		expect(container).toBeEmptyDOMElement()
	})

	it('offers the people who can add books, and not the owner or those who cannot', () => {
		renderLimits()

		const options = within(screen.getByRole('combobox')).getAllByRole('option')
		// nobody is chosen for the owner: the first is the question
		expect(options.map((option) => option.textContent)).toEqual(['Choose a person', 'ana', 'ben'])
	})

	it('shows what the first person was given, and the server’s numbers where they were given none', () => {
		renderLimits()

		expect(boxes().quota.value).toBe('500')
		expect(boxes().maxFile.value).toBe('')
		expect(boxes().maxFile.placeholder).toBe('300')
		expect(boxes().files.placeholder).toBe('50')
		expect(boxes().items.placeholder).toBe('100')
		expect(
			screen.getByText(
				/files up to 300 MB, 50 files in one upload, 2048 MB waiting and 100 books open/,
			),
		).toBeInTheDocument()
	})

	it('shows another person’s limits when they are picked', () => {
		renderLimits()

		fireEvent.change(screen.getByRole('combobox'), { target: { value: 'ben' } })

		expect(boxes().quota.value).toBe('')
		expect(boxes().quota.placeholder).toBe('2048')
	})

	it('saves what was typed, and an empty box follows the server', () => {
		renderLimits()

		type(boxes().files, '20')
		fireEvent.click(screen.getByRole('button', { name: 'Save' }))

		expect(mutate).toHaveBeenCalledTimes(1)
		expect(mutate).toHaveBeenCalledWith({
			input: { maxFileMb: null, maxFilesPerRequest: 20, maxItems: null, quotaMb: 500 },
			userId: 'ana',
		})
	})

	it('lets go of a limit that is emptied', () => {
		renderLimits()

		type(boxes().quota, '')
		fireEvent.click(screen.getByRole('button', { name: 'Save' }))

		expect(mutate).toHaveBeenCalledWith({
			input: { maxFileMb: null, maxFilesPerRequest: null, maxItems: null, quotaMb: null },
			userId: 'ana',
		})
	})

	it('only offers Save once something is different', () => {
		renderLimits()
		const save = screen.getByRole('button', { name: 'Save' })
		expect(save).toBeDisabled()

		type(boxes().items, '10')
		expect(save).toBeEnabled()

		// back to what they have
		type(boxes().items, '')
		expect(save).toBeDisabled()
	})

	it.each(['0', '-5', '1.5', 'many', '1e3'])('refuses %s, and says what is wanted', (value) => {
		renderLimits()

		type(boxes().files, value)

		expect(screen.getByText('A whole number above zero, or empty')).toBeInTheDocument()
		expect(screen.getByRole('button', { name: 'Save' })).toBeDisabled()
		fireEvent.submit(
			screen.getByRole('button', { name: 'Save' }).closest('form') as HTMLFormElement,
		)
		expect(mutate).not.toHaveBeenCalled()
	})

	it('gives a person the server’s limits again, when they were given some of their own', () => {
		renderLimits()

		fireEvent.click(screen.getByRole('button', { name: 'Use the server’s limits' }))

		expect(mutate).toHaveBeenCalledWith({ input: {}, userId: 'ana' })
	})

	it('has nothing to give back to a person who follows the server already', () => {
		renderLimits()

		fireEvent.change(screen.getByRole('combobox'), { target: { value: 'ben' } })

		expect(screen.getByRole('button', { name: 'Use the server’s limits' })).toBeDisabled()
	})

	it('says so when nobody but the owner can add books', () => {
		server.users = [person('owner', [], true), person('carl', [])]

		renderLimits('/settings/book-approvals?person=carl')

		expect(screen.getByText(/Nobody else can add books yet/)).toBeInTheDocument()
		expect(screen.queryByRole('combobox')).not.toBeInTheDocument()
	})
})

describe('Upload limits, closed', () => {
	beforeEach(() => {
		who.canManageUsers = true
		server.users = [person('ana', [UserPermission.AddBooks])]
		server.limits = { ana: limits('ana') }
	})

	it('is a title with a line of help, and nothing else, until it is opened', () => {
		renderLimits('/settings/book-approvals')

		const toggle = screen.getByRole('button', { name: /Upload limits per person/ })
		expect(toggle).toHaveAttribute('aria-expanded', 'false')
		expect(screen.getByText('Leave a box empty to use the server’s limit.')).toBeInTheDocument()
		expect(screen.queryByRole('combobox')).not.toBeInTheDocument()
		expect(screen.queryByLabelText('Biggest file')).not.toBeInTheDocument()
	})

	it('opens on a click, with nobody chosen: the person is asked for', () => {
		renderLimits('/settings/book-approvals')

		fireEvent.click(screen.getByRole('button', { name: /Upload limits per person/ }))

		expect(screen.getByRole('combobox')).toHaveValue('')
		expect(screen.queryByLabelText('Biggest file')).not.toBeInTheDocument()
		fireEvent.change(screen.getByRole('combobox'), { target: { value: 'ana' } })
		expect(screen.getByLabelText('Biggest file')).toBeInTheDocument()
	})

	it('opens by itself, on that person, when it is asked for in the address', () => {
		renderLimits('/settings/book-approvals?person=ana')

		expect(screen.getByRole('button', { name: /Upload limits per person/ })).toHaveAttribute(
			'aria-expanded',
			'true',
		)
		expect(screen.getByRole('combobox')).toHaveValue('ana')
	})

	it('opens by itself at its anchor', () => {
		renderLimits('/settings/book-approvals#limits')

		expect(screen.getByRole('button', { name: /Upload limits per person/ })).toHaveAttribute(
			'aria-expanded',
			'true',
		)
	})
})

describe('a limit that is typed', () => {
	it('is empty (follow the server), a whole number above zero, or nothing a server could take', () => {
		expect(parseLimit('')).toBeNull()
		expect(parseLimit('  ')).toBeNull()
		expect(parseLimit('25')).toBe(25)
		expect(parseLimit(' 25 ')).toBe(25)
		expect(parseLimit('0')).toBeUndefined()
		expect(parseLimit('-1')).toBeUndefined()
		expect(parseLimit('2.5')).toBeUndefined()
		expect(parseLimit('abc')).toBeUndefined()
		expect(parseLimit('99999999999999999999')).toBeUndefined()
	})
})
