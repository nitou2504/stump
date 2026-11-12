import { Alert, AlertDescription, Badge, Button, Card, ConfirmationModal, Input, Text } from '@stump/components'
import { AccessDeviceKind } from '@stump/graphql'
import { EDGE_SIGN_IN_PAGE } from '@stump/sdk'
import { Check, Pencil, X } from 'lucide-react'
import { FormEvent, Suspense, useState } from 'react'
import { Helmet } from 'react-helmet'
import { toast } from 'sonner'

import { SceneContainer } from '@/components/container'
import { useTranslate } from '@/hooks/useTranslate'
import { formatWhen } from '@/scenes/settings/server/access/format'

import { type MyDevice, useMyAccessActions, useMyDevices } from './graphql'

const key = (name: string) => `settingsScene.app/devices.${name}`

const errorMessage = (error: unknown) =>
	error instanceof Error && error.message ? error.message : undefined

function DeviceCard({ device, onRemove }: { device: MyDevice; onRemove: (device: MyDevice) => void }) {
	const { t } = useTranslate()
	const actions = useMyAccessActions()
	const [renaming, setRenaming] = useState(false)
	const [label, setLabel] = useState(device.label)

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
		<Card data-testid={`my-device-${device.id}`} className="gap-3 p-4 flex flex-col">
			<div className="gap-3 flex items-start justify-between">
				<div className="min-w-0">
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
							<Text className="font-medium break-words">{device.label}</Text>
							{device.current && (
								<Badge size="xs" variant="success">
									{t(key('thisDevice'))}
								</Badge>
							)}
							<Badge size="xs" variant="secondary">
								{t(key(device.kind === AccessDeviceKind.Opds ? 'kind.readingApp' : 'kind.browser'))}
							</Badge>
						</div>
					)}
					<Text size="xs" variant="muted" className="mt-1">
						{t(key('detail'), {
							last: device.lastSeenAt ? formatWhen(device.lastSeenAt) : t(key('never')),
							ip: device.lastIp || '–',
						})}
					</Text>
				</div>
				{!renaming && (
					<Button
						size="sm"
						variant="ghost"
						onClick={() => {
							setLabel(device.label)
							setRenaming(true)
						}}
					>
						<Pencil className="h-4 w-4" />
						<span className="sr-only">{t(key('rename.start'), { name: device.label })}</span>
					</Button>
				)}
			</div>
			<div>
				<Button size="sm" variant="outline" disabled={actions.isPending} onClick={() => onRemove(device)}>
					{t(key(device.current ? 'removeAndSignOut' : 'remove'))}
				</Button>
			</div>
		</Card>
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
			// the device you are on is gone, and so is your session: the sign-in page is where this ends
			if (device.current) window.location.assign(EDGE_SIGN_IN_PAGE)
		} catch (error) {
			toast.error(t(key('errors.remove')), { description: errorMessage(error) })
		}
	}

	return (
		<div className="gap-4 flex flex-col">
			<Text size="sm" variant="muted" data-testid="my-devices-count">
				{t(key('count'), { active, limit })}
			</Text>
			{active >= limit && (
				<Alert variant="warning" data-testid="my-devices-full">
					<AlertDescription>{t(key('full'), { limit })}</AlertDescription>
				</Alert>
			)}
			{devices.length === 0 && (
				<Text size="sm" variant="muted">
					{t(key('empty'))}
				</Text>
			)}
			<div className="gap-3 md:grid-cols-2 grid grid-cols-1">
				{devices.map((device) => (
					<DeviceCard key={device.id} device={device} onRemove={setRemoving} />
				))}
			</div>
			<Text size="xs" variant="muted">
				{t(key('footer'), { limit })}
			</Text>

			<ConfirmationModal
				title={t(key(removing?.current ? 'confirm.currentTitle' : 'confirm.title'))}
				description={t(key(removing?.current ? 'confirm.currentDescription' : 'confirm.description'), {
					name: removing?.label ?? '',
				})}
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
 * *My devices*: the browsers a person has signed in on and the reading apps they use, each with the
 * name they can change and a button to remove it. Removing one stops it working within seconds;
 * removing the one you are on also signs you out. A person only ever sees their own
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
