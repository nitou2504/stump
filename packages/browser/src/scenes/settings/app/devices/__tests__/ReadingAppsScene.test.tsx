import '@/__mocks__/resizeObserver'

import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import { sent, setup } from '@/scenes/settings/server/access/__tests__/support'

import ReadingAppsScene from '../ReadingAppsScene'

vi.mock('@/hooks/useTranslate', () => import('@/scenes/settings/server/access/__tests__/translations'))
vi.mock('react-helmet', () => ({ Helmet: () => null }))
vi.mock('@/components/container', () => ({
	SceneContainer: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
}))
vi.mock('@/scenes/settings/server/access/copy', () => ({ copyText: vi.fn().mockResolvedValue(true) }))
vi.mock('@/scenes/settings/server/access/QrCode', () => ({
	default: ({ value }: { value: string }) => <div data-testid="qr-stand-in">{value}</div>,
}))
vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn(), info: vi.fn() } }))

import { toast } from 'sonner'

import { copyText } from '@/scenes/settings/server/access/copy'

const K = (key: string) => `settingsScene.app/readingApps.${key}`

const ADDRESS = 'https://g3.test:8443/opds/kobo-9x2mq.KEYKEYKEYKEYKEYKEYKEYKEYKEYKEYKEYKEYKEYKEYK/v1.2/catalog'

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
}: { apps?: ReturnType<typeof app1>[]; active?: number; limit?: number; canDownload?: boolean } = {}) {
	const app = setup({
		context: {
			isServerOwner: false,
			user: { id: 'ana-id', isServerOwner: false },
			checkPermission: (permission: string) => canDownload || permission !== 'DOWNLOAD_FILE',
		},
		answers: (query) => {
			if (query.includes('query MyReadingApps')) return { myReadingApps: apps }
			if (query.includes('query MyDevices')) return { myDevices: { devices: [], active, limit } }
			if (query.includes('mutation CreateReadingApp')) return { createReadingApp: app1({ id: 'new' }) }
			if (query.includes('mutation ReplaceReadingApp')) return { replaceReadingApp: app1({ id: 'newer' }) }
			if (query.includes('mutation RemoveReadingApp')) return { removeReadingApp: true }
			throw new Error(`Unexpected query ${query}`)
		},
	})
	render(<ReadingAppsScene />, { wrapper: app.Wrapper })
	return app
}

describe('Reading apps', () => {
	beforeEach(() => {
		vi.clearAllMocks()
	})

	it('shows the address of each app to copy, the steps to take and the QR code on request', async () => {
		renderScene({ apps: [app1()] })
		const user = userEvent.setup()

		const card = await screen.findByTestId('reading-app-kobo-9x2mq')
		expect(within(card).getByLabelText(K('address'))).toHaveValue(ADDRESS)
		for (const step of ['0', '1', '2']) {
			expect(within(card).getByText(K(`steps.${step}`))).toBeInTheDocument()
		}
		expect(within(card).queryByTestId('qr-stand-in')).toBeNull()

		await user.click(within(card).getByRole('button', { name: K('copy') }))
		expect(copyText).toHaveBeenCalledWith(ADDRESS)
		expect(toast.success).toHaveBeenCalledWith(K('copied'))

		await user.click(within(card).getByRole('button', { name: K('qr') }))
		expect(within(card).getByTestId('qr-stand-in')).toHaveTextContent(ADDRESS)
	})

	it('adds a reading app with the name it was given', async () => {
		const { execute } = renderScene()
		const user = userEvent.setup()

		await user.type(await screen.findByLabelText(K('name')), '  My Kobo ')
		await user.click(screen.getByRole('button', { name: K('add') }))

		await waitFor(() => expect(sent(execute, 'CreateReadingApp')).toEqual([{ label: 'My Kobo' }]))
	})

	it('cannot add one without a name, or once every device is in use, and says why', async () => {
		renderScene({ active: 3, limit: 3 })

		expect(await screen.findByTestId('reading-apps-full')).toHaveTextContent(`${K('full')} limit=3`)
		expect(screen.getByLabelText(K('name'))).toBeDisabled()
		expect(screen.getByRole('button', { name: K('add') })).toBeDisabled()
	})

	it('is closed to an account that can not download, with the reason', async () => {
		renderScene({ canDownload: false })

		expect(await screen.findByTestId('reading-apps-no-download')).toHaveTextContent(K('noDownload'))
		expect(screen.getByLabelText(K('name'))).toBeDisabled()
		expect(screen.getByRole('button', { name: K('add') })).toBeDisabled()
	})

	it('replaces an address after asking, and removes an app after asking', async () => {
		const { execute } = renderScene({ apps: [app1()] })
		const user = userEvent.setup()
		const card = await screen.findByTestId('reading-app-kobo-9x2mq')

		await user.click(within(card).getByRole('button', { name: K('replace') }))
		let dialog = await screen.findByRole('dialog')
		expect(within(dialog).getByText(`${K('confirm.replaceDescription')} name=Kobo`)).toBeInTheDocument()
		await user.click(within(dialog).getByRole('button', { name: K('replace') }))
		await waitFor(() => expect(sent(execute, 'ReplaceReadingApp')).toEqual([{ id: 'kobo-9x2mq' }]))
		await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull())

		await user.click(within(card).getByRole('button', { name: K('remove') }))
		dialog = await screen.findByRole('dialog')
		await user.click(within(dialog).getByRole('button', { name: K('remove') }))
		await waitFor(() => expect(sent(execute, 'RemoveReadingApp')).toEqual([{ id: 'kobo-9x2mq' }]))
	})
})
