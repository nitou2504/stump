import '@/__mocks__/resizeObserver'

import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import { sent, setup } from '@/scenes/settings/server/access/__tests__/support'

import MyDevicesScene from '../MyDevicesScene'

vi.mock('@/hooks/useTranslate', () => import('@/scenes/settings/server/access/__tests__/translations'))
vi.mock('react-helmet', () => ({ Helmet: () => null }))
vi.mock('@/components/container', () => ({
	SceneContainer: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
}))
vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn(), info: vi.fn() } }))

import { toast } from 'sonner'

const K = (key: string) => `settingsScene.app/devices.${key}`

const device = (id: string, label: string, extra: Record<string, unknown> = {}) => ({
	id,
	label,
	kind: 'BROWSER',
	current: false,
	createdAt: '2026-11-10T10:00:00Z',
	lastSeenAt: '2026-11-11T10:00:00Z',
	lastIp: '198.51.100.7',
	...extra,
})

function renderScene({ devices, active = devices.length, limit = 3, removed = true }: {
	devices: ReturnType<typeof device>[]
	active?: number
	limit?: number
	removed?: boolean
}) {
	const app = setup({
		context: { isServerOwner: false, user: { id: 'ana-id', isServerOwner: false } },
		answers: (query) => {
			if (query.includes('query MyDevices')) return { myDevices: { devices, active, limit } }
			if (query.includes('mutation RenameMyDevice'))
				return { renameMyDevice: { id: 'x', label: 'y' } }
			if (query.includes('mutation RemoveMyDevice')) return { removeMyDevice: removed }
			throw new Error(`Unexpected query ${query}`)
		},
	})
	render(<MyDevicesScene />, { wrapper: app.Wrapper })
	return app
}

describe('My devices', () => {
	const assign = vi.fn()
	const original = window.location

	beforeEach(() => {
		vi.clearAllMocks()
		Object.defineProperty(window, 'location', {
			configurable: true,
			value: { pathname: '/settings/devices', assign },
		})
	})

	afterAll(() => {
		Object.defineProperty(window, 'location', { configurable: true, value: original })
	})

	it('lists the devices with their kind, where they were last used, and marks the one in use', async () => {
		renderScene({
			devices: [
				device('a', 'Chrome on Android', { current: true }),
				device('b', 'Kobo', { kind: 'OPDS', lastSeenAt: null }),
			],
		})

		const current = await screen.findByTestId('my-device-a')
		expect(within(current).getByText(K('thisDevice'))).toBeInTheDocument()
		expect(within(current).getByText(K('kind.browser'))).toBeInTheDocument()
		expect(within(current).getByRole('button', { name: K('removeAndSignOut') })).toBeInTheDocument()
		const kobo = screen.getByTestId('my-device-b')
		expect(within(kobo).getByText(K('kind.readingApp'))).toBeInTheDocument()
		expect(within(kobo).queryByText(K('thisDevice'))).toBeNull()
		expect(kobo.textContent).toContain(`${K('detail')} last=${K('never')} ip=198.51.100.7`)
		expect(screen.getByTestId('my-devices-count')).toHaveTextContent(`${K('count')} active=2 limit=3`)
		expect(screen.queryByTestId('my-devices-full')).toBeNull()
	})

	it('says so when every device is in use', async () => {
		renderScene({ devices: [device('a', 'One'), device('b', 'Two'), device('c', 'Three')], limit: 3 })

		expect(await screen.findByTestId('my-devices-full')).toHaveTextContent(`${K('full')} limit=3`)
	})

	it('renames a device, trimming the name', async () => {
		const { execute } = renderScene({ devices: [device('a', 'Chrome on Android')] })
		const user = userEvent.setup()

		const card = await screen.findByTestId('my-device-a')
		await user.click(within(card).getByRole('button', { name: `${K('rename.start')} name=Chrome on Android` }))
		const field = within(card).getByLabelText(K('rename.label'))
		await user.clear(field)
		await user.type(field, '  Kitchen tablet ')
		await user.click(within(card).getByRole('button', { name: K('rename.save') }))

		await waitFor(() =>
			expect(sent(execute, 'RenameMyDevice')).toEqual([{ id: 'a', label: 'Kitchen tablet' }]),
		)
	})

	it('removes another device after asking, and stays on the page', async () => {
		const { execute } = renderScene({
			devices: [device('a', 'Chrome on Android', { current: true }), device('b', 'Old laptop')],
		})
		const user = userEvent.setup()

		await user.click(
			within(await screen.findByTestId('my-device-b')).getByRole('button', { name: K('remove') }),
		)
		const dialog = await screen.findByRole('alertdialog')
		expect(within(dialog).getByText(`${K('confirm.description')} name=Old laptop`)).toBeInTheDocument()
		await user.click(within(dialog).getByRole('button', { name: K('remove') }))

		await waitFor(() => expect(sent(execute, 'RemoveMyDevice')).toEqual([{ id: 'b' }]))
		expect(toast.success).toHaveBeenCalledWith(`${K('removed')} name=Old laptop`)
		expect(assign).not.toHaveBeenCalled()
	})

	it('removes the device in use and goes to the sign-in page by address, the session being gone', async () => {
		const { execute } = renderScene({ devices: [device('a', 'Chrome on Android', { current: true })] })
		const user = userEvent.setup()

		await user.click(
			within(await screen.findByTestId('my-device-a')).getByRole('button', {
				name: K('removeAndSignOut'),
			}),
		)
		const dialog = await screen.findByRole('alertdialog')
		expect(within(dialog).getByText(K('confirm.currentTitle'))).toBeInTheDocument()
		await user.click(within(dialog).getByRole('button', { name: K('removeAndSignOut') }))

		await waitFor(() => expect(sent(execute, 'RemoveMyDevice')).toEqual([{ id: 'a' }]))
		// not at once: the edge needs a moment to learn the pass is revoked
		expect(assign).not.toHaveBeenCalled()
		await waitFor(() => expect(assign).toHaveBeenCalledExactlyOnceWith('/api/_i/signin'), {
			timeout: 5000,
		})
	}, 15_000)

	it('does not leave the page, and says so, when the server removed nothing', async () => {
		renderScene({ devices: [device('a', 'Chrome on Android', { current: true })], removed: false })
		const user = userEvent.setup()

		await user.click(
			within(await screen.findByTestId('my-device-a')).getByRole('button', {
				name: K('removeAndSignOut'),
			}),
		)
		await user.click(
			within(await screen.findByRole('alertdialog')).getByRole('button', { name: K('removeAndSignOut') }),
		)

		await waitFor(() => expect(toast.error).toHaveBeenCalledWith(K('errors.remove')))
		expect(toast.success).not.toHaveBeenCalled()
		expect(assign).not.toHaveBeenCalled()
	})
})
