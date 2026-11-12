import '@/__mocks__/resizeObserver'

import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import { sent, setup } from '@/scenes/settings/server/access/__tests__/support'

import ReadingAppsScene from '../ReadingAppsScene'

vi.mock(
	'@/hooks/useTranslate',
	() => import('@/scenes/settings/server/access/__tests__/translations'),
)
vi.mock('react-helmet', () => ({ Helmet: () => null }))
vi.mock('@/components/container', () => ({
	SceneContainer: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
}))
vi.mock('@/scenes/settings/server/access/copy', () => ({
	copyText: vi.fn().mockResolvedValue(true),
}))
vi.mock('@/scenes/settings/server/access/QrCode', () => ({
	default: ({ value }: { value: string }) => <div data-testid="qr-stand-in">{value}</div>,
}))
vi.mock('@/paths', () => ({
	usePaths: () => ({ settings: (page: string) => `/settings/${page}` }),
}))
vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn(), info: vi.fn() } }))

import { toast } from 'sonner'

import { copyText } from '@/scenes/settings/server/access/copy'

const K = (key: string) => `settingsScene.app/readingApps.${key}`

const ADDRESS =
	'https://g3.test:8443/opds/kobo-9x2mq.KEYKEYKEYKEYKEYKEYKEYKEYKEYKEYKEYKEYKEYKEYK/v1.2/catalog'

const app1 = (extra: Record<string, unknown> = {}) => ({
	id: 'kobo-9x2mq',
	label: 'Kobo',
	url: ADDRESS,
	createdAt: '2026-11-10T10:00:00Z',
	lastSeenAt: null,
	...extra,
})

function renderScene({
	apps = [] as ReturnType<typeof app1>[],
	active = apps.length + 1,
	limit = 3,
	canDownload = true,
}: {
	apps?: ReturnType<typeof app1>[]
	active?: number
	limit?: number
	canDownload?: boolean
} = {}) {
	const app = setup({
		context: {
			isServerOwner: false,
			user: { id: 'ana-id', isServerOwner: false },
			checkPermission: (permission: string) => canDownload || permission !== 'DOWNLOAD_FILE',
		},
		answers: (query) => {
			if (query.includes('query MyReadingApps')) return { myReadingApps: apps }
			if (query.includes('query MyDevices')) return { myDevices: { devices: [], active, limit } }
			if (query.includes('mutation CreateReadingApp'))
				return { createReadingApp: app1({ id: 'new' }) }
			if (query.includes('mutation ReplaceReadingApp'))
				return { replaceReadingApp: app1({ id: 'newer' }) }
			if (query.includes('mutation RemoveReadingApp')) return { removeReadingApp: true }
			throw new Error(`Unexpected query ${query}`)
		},
	})
	render(<ReadingAppsScene />, { wrapper: app.Wrapper })
	return app
}

/** Opens the menu of an app and picks one of its entries */
async function choose(user: ReturnType<typeof userEvent.setup>, name: string, entry: string) {
	const row = await screen.findByTestId('reading-app-kobo-9x2mq')
	await user.click(
		await within(row).findByRole('button', { name: `${K('moreActions')} name=${name}` }),
	)
	await user.click(await screen.findByRole('menuitem', { name: entry }))
}

describe('Reading apps', () => {
	beforeEach(() => {
		vi.clearAllMocks()
	})

	it('is a list and one button: no form, no address and no instructions on the first view', async () => {
		renderScene({ apps: [app1()] })

		const row = await screen.findByTestId('reading-app-kobo-9x2mq')
		expect(row).toHaveTextContent('Kobo')
		expect(row).toHaveTextContent(K('neverUsed'))
		expect(screen.queryByTestId('reading-app-form')).toBeNull()
		expect(document.body.textContent).not.toContain('KEYKEY')
		expect(screen.queryByText(K('steps.0'))).toBeNull()
		const main = [...document.querySelectorAll('button')].filter((button) =>
			button.className.split(' ').includes('bg-primary'),
		)
		expect(main).toHaveLength(1)
		expect(main[0]).toHaveTextContent(K('add'))
	})

	it('shows the address of an app only when asked, with the steps, a copy button and the QR code on request', async () => {
		renderScene({ apps: [app1()] })
		const user = userEvent.setup()

		const row = await screen.findByTestId('reading-app-kobo-9x2mq')
		await user.click(within(row).getByRole('button', { name: K('showAddress') }))

		const dialog = await screen.findByTestId('reading-app-address')
		expect(within(dialog).getByText(`${K('addressFor')} name=Kobo`)).toBeInTheDocument()
		expect(within(dialog).getByLabelText(K('address'))).toHaveValue(ADDRESS)
		for (const step of ['0', '1', '2']) {
			expect(within(dialog).getByText(K(`steps.${step}`))).toBeInTheDocument()
		}
		expect(within(dialog).getByText(K('keepPrivate'))).toBeInTheDocument()
		expect(within(dialog).queryByTestId('qr-stand-in')).toBeNull()

		await user.click(within(dialog).getByRole('button', { name: K('copy') }))
		expect(copyText).toHaveBeenCalledWith(ADDRESS)
		expect(toast.success).toHaveBeenCalledWith(K('copied'))

		await user.click(within(dialog).getByRole('button', { name: K('showQr') }))
		expect(within(dialog).getByTestId('qr-stand-in')).toHaveTextContent(ADDRESS)
		// closing it takes the address off the page
		await user.click(within(dialog).getByRole('button', { name: K('done') }))
		await waitFor(() => expect(screen.queryByTestId('reading-app-address')).toBeNull())
	})

	it('adds a reading app with the name it was given, and then shows its address', async () => {
		const { execute } = renderScene()
		const user = userEvent.setup()

		await user.click(await screen.findByRole('button', { name: K('add') }))
		await user.type(await screen.findByLabelText(K('name')), '  My Kobo ')
		await user.click(screen.getByRole('button', { name: K('addConfirm') }))

		await waitFor(() => expect(sent(execute, 'CreateReadingApp')).toEqual([{ label: 'My Kobo' }]))
		const dialog = await screen.findByTestId('reading-app-address')
		expect(within(dialog).getByLabelText(K('address'))).toHaveValue(ADDRESS)
	})

	it('cannot add one without a name', async () => {
		renderScene()
		const user = userEvent.setup()

		await user.click(await screen.findByRole('button', { name: K('add') }))
		expect(await screen.findByRole('button', { name: K('addConfirm') })).toBeDisabled()
	})

	it('cannot add one once every device is in use, and says where to make room', async () => {
		renderScene({ active: 3, limit: 3 })

		const full = await screen.findByTestId('reading-apps-full')
		expect(full).toHaveTextContent(`${K('full')} limit=3`)
		expect(within(full).getByRole('link', { name: K('myDevices') })).toHaveAttribute(
			'href',
			'/settings/devices',
		)
		expect(screen.getByRole('button', { name: K('add') })).toBeDisabled()
	})

	it('is closed to an account that can not download, with the reason', async () => {
		renderScene({ canDownload: false })

		expect(await screen.findByTestId('reading-apps-no-download')).toHaveTextContent(K('noDownload'))
		expect(screen.getByRole('button', { name: K('add') })).toBeDisabled()
	})

	it('gives a new address after asking, and shows it', async () => {
		const { execute } = renderScene({ apps: [app1()] })
		const user = userEvent.setup()

		await choose(user, 'Kobo', K('replace'))
		const dialog = await screen.findByRole('alertdialog')
		expect(
			within(dialog).getByText(`${K('confirm.replaceDescription')} name=Kobo`),
		).toBeInTheDocument()
		expect(sent(execute, 'ReplaceReadingApp')).toEqual([])
		await user.click(within(dialog).getByRole('button', { name: K('replace') }))

		await waitFor(() => expect(sent(execute, 'ReplaceReadingApp')).toEqual([{ id: 'kobo-9x2mq' }]))
		// the new address is what they came for
		const shown = await screen.findByTestId('reading-app-address')
		expect(within(shown).getByText(`${K('addressFor')} name=Kobo`)).toBeInTheDocument()
	})

	it('removes an app after asking', async () => {
		const { execute } = renderScene({ apps: [app1()] })
		const user = userEvent.setup()

		await choose(user, 'Kobo', K('remove'))
		const dialog = await screen.findByRole('alertdialog')
		expect(sent(execute, 'RemoveReadingApp')).toEqual([])
		await user.click(within(dialog).getByRole('button', { name: K('remove') }))

		await waitFor(() => expect(sent(execute, 'RemoveReadingApp')).toEqual([{ id: 'kobo-9x2mq' }]))
	})
})
