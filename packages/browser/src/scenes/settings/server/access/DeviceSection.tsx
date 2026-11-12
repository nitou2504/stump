import { Badge, Button, cn, ConfirmationModal, Text } from '@stump/components'
import { AccessDeviceKind, AccessDeviceStatus } from '@stump/graphql'
import { Suspense, useMemo, useState, useTransition } from 'react'
import { toast } from 'sonner'

import { useTranslate } from '@/hooks/useTranslate'

import { formatWhen, shortAgent } from './format'
import { type AccessDevice, useAccessActions, useAccessDevices } from './graphql'
import { accessKey } from './keys'
import Pager from './Pager'
import PersonLabel from './PersonLabel'
import Section, { Empty, Row, RowList } from './Section'

const errorMessage = (error: unknown) =>
	error instanceof Error && error.message ? error.message : undefined

/** The devices of one person on the page, or the ones made in the terminal, which are nobody's */
type Group = {
	key: string
	person: AccessDevice['person']
	devices: AccessDevice[]
}

/** In the order a person first shows up in, so a page doesn't jump about when it is reloaded */
export function groupByPerson(devices: AccessDevice[]): Group[] {
	const groups = new Map<string, Group>()
	for (const device of devices) {
		const key = device.person?.id ?? 'terminal'
		const group = groups.get(key) ?? { key, person: device.person, devices: [] }
		group.devices.push(device)
		groups.set(key, group)
	}
	return [...groups.values()]
}

type Pending =
	| { kind: 'device'; id: string; label: string }
	| { kind: 'person'; id: string; name: string }

function DeviceList() {
	const { t } = useTranslate()
	const [page, setPage] = useState(1)
	// the page that is shown stays until the next one is ready
	const [isPaging, startTransition] = useTransition()
	const { nodes, pageInfo } = useAccessDevices(page)
	const actions = useAccessActions()
	const [pending, setPending] = useState<Pending | null>(null)

	const groups = useMemo(() => groupByPerson(nodes), [nodes])

	const run = async (work: () => Promise<unknown>, done: string, failed: string) => {
		try {
			await work()
			toast.success(done)
		} catch (error) {
			toast.error(failed, { description: errorMessage(error) })
		}
	}

	const confirm = async () => {
		if (!pending) return
		if (pending.kind === 'device') {
			await run(
				() => actions.revokeDevice(pending.id),
				t(accessKey('devices.revokedDevice'), { label: pending.label }),
				t(accessKey('devices.errors.revoke')),
			)
		} else {
			await run(
				async () => {
					const count = await actions.revokePerson(pending.id)
					toast.info(t(accessKey('devices.revokedCount'), { count }))
				},
				t(accessKey('devices.revokedPerson'), { name: pending.name }),
				t(accessKey('devices.errors.revoke')),
			)
		}
		setPending(null)
	}

	return (
		<div className={cn('gap-y-4 flex flex-col', { 'opacity-60': isPaging })} aria-busy={isPaging}>
			{groups.length === 0 && (
				<RowList>
					<Empty>{t(accessKey('devices.empty'))}</Empty>
				</RowList>
			)}

			{groups.map((group) => {
				const active = group.devices.filter((device) => device.status === AccessDeviceStatus.Active)
				return (
					<div
						key={group.key}
						data-testid={`access-person-${group.key}`}
						className="gap-2 flex flex-col"
					>
						<div className="gap-2 flex flex-wrap items-center justify-between">
							{group.person ? (
								<PersonLabel person={group.person} />
							) : (
								<Text size="sm" className="font-medium">
									{t(accessKey('devices.terminalGroup'))}
								</Text>
							)}
							{group.person && active.length > 0 && (
								<Button
									size="sm"
									variant="destructive"
									disabled={actions.isPending}
									onClick={() =>
										setPending({
											kind: 'person',
											id: group.person!.id,
											name: group.person!.username,
										})
									}
								>
									{t(accessKey('devices.revokeAll'), { name: group.person.username })}
								</Button>
							)}
						</div>

						<RowList>
							{group.devices.map((device) => {
								const isActive = device.status === AccessDeviceStatus.Active
								return (
									<Row key={device.id} data-testid={`access-device-${device.id}`}>
										<div className="min-w-0">
											<div className="gap-2 flex flex-wrap items-center">
												<Text size="sm" className="font-medium">
													{device.label}
												</Text>
												{device.legacy && (
													<Badge size="xs" variant="secondary">
														{t(accessKey('devices.legacy'))}
													</Badge>
												)}
												{device.kind === AccessDeviceKind.Opds && (
													<Badge size="xs" variant="primary">
														{t(accessKey('devices.readingApp'))}
													</Badge>
												)}
												{!isActive && (
													<Badge size="xs" variant="error">
														{t(accessKey('devices.revoked'))}
													</Badge>
												)}
											</div>
											<Text
												size="xs"
												variant="muted"
												className="mt-1"
												title={device.lastAgent || undefined}
											>
												{t(accessKey('devices.detail'), {
													first: formatWhen(device.firstSeenAt),
													last: formatWhen(device.lastSeenAt),
													ip: device.lastIp || '–',
												})}
												{device.lastAgent ? ` · ${shortAgent(device.lastAgent)}` : ''}
											</Text>
										</div>
										<div className="gap-2 flex flex-wrap">
											{isActive ? (
												<>
													<Button
														size="sm"
														variant="ghost"
														disabled={actions.isPending}
														onClick={() =>
															run(
																() => actions.resetBudget(device.id),
																t(accessKey('devices.budgetReset'), { label: device.label }),
																t(accessKey('devices.errors.budget')),
															)
														}
													>
														{t(accessKey('devices.resetBudget'))}
													</Button>
													<Button
														size="sm"
														variant="outline"
														disabled={actions.isPending}
														onClick={() =>
															setPending({ kind: 'device', id: device.id, label: device.label })
														}
													>
														{t(accessKey('devices.revoke'))}
													</Button>
												</>
											) : (
												<Button
													size="sm"
													variant="outline"
													disabled={actions.isPending}
													onClick={() =>
														run(
															() => actions.restoreDevice(device.id),
															t(accessKey('devices.restoredDevice'), { label: device.label }),
															t(accessKey('devices.errors.restore')),
														)
													}
												>
													{t(accessKey('devices.restore'))}
												</Button>
											)}
										</div>
									</Row>
								)
							})}
						</RowList>
					</div>
				)
			})}

			<Pager
				page={page}
				totalPages={pageInfo.totalPages}
				onChange={(next) => startTransition(() => setPage(next))}
			/>

			<ConfirmationModal
				title={t(
					accessKey(
						pending?.kind === 'person' ? 'devices.confirmPersonTitle' : 'devices.confirmTitle',
					),
				)}
				description={
					pending?.kind === 'person'
						? t(accessKey('devices.confirmPersonDescription'), { name: pending.name })
						: t(accessKey('devices.confirmDescription'), {
								label: pending?.kind === 'device' ? pending.label : '',
							})
				}
				isOpen={!!pending}
				onClose={() => setPending(null)}
				onConfirm={confirm}
				confirmIsLoading={actions.isPending}
				confirmText={t(accessKey('devices.confirmRevoke'))}
				confirmVariant="destructive"
			/>
		</div>
	)
}

/** The devices that hold a pass: who, when they were last seen, from where, and the way to cut them off */
export default function DeviceSection() {
	const { t } = useTranslate()

	return (
		<Section
			id="devices"
			title={t(accessKey('devices.title'))}
			description={t(accessKey('devices.description'))}
		>
			<Suspense fallback={null}>
				<DeviceList />
			</Suspense>
		</Section>
	)
}
