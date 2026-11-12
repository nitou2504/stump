import {
	Alert,
	AlertDescription,
	Badge,
	Button,
	ConfirmationModal,
	DropdownMenu,
	Input,
	NewCard,
	Text,
} from '@stump/components'
import { AccessDeviceKind } from '@stump/graphql'
import { EDGE_SIGN_IN_PAGE } from '@stump/sdk'
import { BookOpen, Check, Monitor, MoreHorizontal, X } from 'lucide-react'
import { FormEvent, Suspense, useState } from 'react'
import { Helmet } from 'react-helmet'
import { toast } from 'sonner'

import { SceneContainer } from '@/components/container'
import { useTranslate } from '@/hooks/useTranslate'
import { formatAgo, formatWhen } from '@/scenes/settings/server/access/format'

import { type MyDevice, useMyAccessActions, useMyDevices } from './graphql'

/** How long the page waits after the device in use is removed (see `confirm`) */
const SIGN_OUT_DELAY_MS = 2500

const key = (name: string) => `settingsScene.app/devices.${name}`

const errorMessage = (error: unknown) =>
	error instanceof Error && error.message ? error.message : undefined

/** One device: its name, what kind it is, when it was used, and one menu for what can be done */
function DeviceRow({
	device,
	onRemove,
}: {
	device: MyDevice
	onRemove: (device: MyDevice) => void
}) {
	const { t } = useTranslate()
	const actions = useMyAccessActions()
	const [renaming, setRenaming] = useState(false)
	const [label, setLabel] = useState(device.label)
	const isApp = device.kind === AccessDeviceKind.Opds
	const Icon = isApp ? BookOpen : Monitor

	const save = async (event: FormEvent) => {
		event.preventDefault()
		const next = label.trim()
		if (!next) return
		try {
			await actions.renameDevice(device.id, next)
			setRenaming(false)
		} catch (error) {
			toast.error(t(key('errors.rename')), { description: errorMessage(error) })
		}
	}

	return (
		<div data-testid={`my-device-${device.id}`} className="gap-3 px-4 py-3 flex items-center">
			<Icon aria-hidden className="h-4 w-4 shrink-0 text-muted-foreground" />
			<div className="min-w-0 flex-1">
				{renaming ? (
					<form onSubmit={save} className="gap-2 flex items-center">
						<Input
							aria-label={t(key('rename.label'))}
							value={label}
							onChange={(event) => setLabel(event.target.value)}
							maxLength={40}
							autoFocus
						/>
						<Button type="submit" size="sm" disabled={!label.trim() || actions.isPending}>
							<Check className="h-4 w-4" />
							<span className="sr-only">{t(key('rename.save'))}</span>
						</Button>
						<Button
							type="button"
							size="sm"
							variant="ghost"
							onClick={() => {
								setLabel(device.label)
								setRenaming(false)
							}}
						>
							<X className="h-4 w-4" />
							<span className="sr-only">{t(key('rename.cancel'))}</span>
						</Button>
					</form>
				) : (
					<div className="gap-2 flex flex-wrap items-center">
						<Text size="sm" className="font-medium break-words">
							{device.label}
						</Text>
						{device.current && (
							<Badge size="xs" variant="success">
								{t(key('thisDevice'))}
							</Badge>
						)}
						<Badge size="xs" variant="secondary">
							{t(key(isApp ? 'kind.readingApp' : 'kind.browser'))}
						</Badge>
					</div>
				)}
				<Text size="xs" variant="muted" title={formatWhen(device.lastSeenAt)}>
					{device.lastSeenAt
						? t(key('used'), { when: formatAgo(device.lastSeenAt) })
						: t(key('neverUsed'))}
				</Text>
			</div>
			{!renaming && (
				<DropdownMenu
					align="end"
					trigger={
						<Button
							variant="ghost"
							size="icon"
							className="shrink-0"
							aria-label={t(key('moreActions'), { name: device.label })}
						>
							<MoreHorizontal className="h-4 w-4" />
						</Button>
					}
					groups={[
						{
							items: [
								{
									label: t(key('rename.start')),
									onClick: () => {
										setLabel(device.label)
										setRenaming(true)
									},
								},
								{
									label: t(key(device.current ? 'removeAndSignOut' : 'remove')),
									variant: 'destructive',
									onClick: () => onRemove(device),
								},
							],
						},
					]}
				/>
			)}
		</div>
	)
}

function DeviceList() {
	const { t } = useTranslate()
	const { devices, active, limit } = useMyDevices()
	const actions = useMyAccessActions()
	const [removing, setRemoving] = useState<MyDevice | null>(null)

	const confirm = async () => {
		if (!removing) return
		const device = removing
		try {
			const removed = await actions.removeDevice(device.id)
			setRemoving(null)
			// the server found nothing of that name to remove (it was gone already): the list is reloaded
			if (!removed) {
				toast.error(t(key('errors.remove')))
				return
			}
			toast.success(t(key('removed'), { name: device.label }))
			// the device you are on is gone, and so is your session: the sign-in page is where this ends. A
			// moment is given first: the edge follows the app's state file once a second, and until it has
			// the pass is still honoured there, so a page opened at once would be served as a signed-in
			// visit and could meet the old pass again
			if (device.current) {
				window.setTimeout(() => window.location.assign(EDGE_SIGN_IN_PAGE), SIGN_OUT_DELAY_MS)
			}
		} catch (error) {
			toast.error(t(key('errors.remove')), { description: errorMessage(error) })
		}
	}

	return (
		<div className="gap-4 flex flex-col">
			{active >= limit ? (
				<Alert variant="warning" data-testid="my-devices-full">
					<AlertDescription>{t(key('full'), { limit })}</AlertDescription>
				</Alert>
			) : (
				<Text size="sm" variant="muted" data-testid="my-devices-count">
					{t(key('count'), { active, limit })}
				</Text>
			)}

			{devices.length === 0 ? (
				<Text size="sm" variant="muted">
					{t(key('empty'))}
				</Text>
			) : (
				<NewCard>
					<div className="divide-y divide-border/70">
						{devices.map((device) => (
							<DeviceRow key={device.id} device={device} onRemove={setRemoving} />
						))}
					</div>
				</NewCard>
			)}
			<Text size="xs" variant="muted">
				{t(key('footer'))}
			</Text>

			<ConfirmationModal
				title={t(key(removing?.current ? 'confirm.currentTitle' : 'confirm.title'))}
				description={t(
					key(removing?.current ? 'confirm.currentDescription' : 'confirm.description'),
					{ name: removing?.label ?? '' },
				)}
				isOpen={!!removing}
				onClose={() => setRemoving(null)}
				onConfirm={confirm}
				confirmIsLoading={actions.isPending}
				confirmText={t(key(removing?.current ? 'removeAndSignOut' : 'remove'))}
				confirmVariant="destructive"
			/>
		</div>
	)
}

/**
 * *My devices*: the browsers a person has signed in on and the reading apps they use, in one list
 * with one line on how many they use. Each can be renamed or removed from its menu; removing one
 * stops it working within seconds, and removing the one you are on also signs you out. A person
 * only ever sees their own
 */
export default function MyDevicesScene() {
	const { t } = useTranslate()

	return (
		<SceneContainer variant="settings">
			<Helmet>
				<title>Stump | {t(key('helmet'))}</title>
			</Helmet>
			<Suspense fallback={null}>
				<DeviceList />
			</Suspense>
		</SceneContainer>
	)
}
