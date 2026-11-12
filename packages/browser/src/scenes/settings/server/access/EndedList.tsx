import { Badge, Button, DropdownMenu, Text } from '@stump/components'
import { AccessDeviceStatus, AccessInviteStatus, SignupLinkStatus } from '@stump/graphql'
import { MoreHorizontal } from 'lucide-react'
import { useState } from 'react'
import { toast } from 'sonner'

import { useTranslate } from '@/hooks/useTranslate'

import { oldPasses } from './attention'
import { formatWhen } from './format'
import {
	type AccessDevice,
	type AccessInvite,
	type AccessSignupLink,
	useAccessActions,
	useAccessDevices,
	useAccessInvites,
	useAccessSignupLinks,
	useAllAccessDevices,
} from './graphql'
import { accessKey } from './keys'
import RevokeLinkDialog, { type Revoking } from './RevokeLinkDialog'

const errorMessage = (error: unknown) =>
	error instanceof Error && error.message ? error.message : undefined

/** Links that no longer work: used up, expired or revoked */
const endedLinks = (links: readonly AccessSignupLink[]) =>
	links.filter(({ status }) => status !== SignupLinkStatus.Active)

const endedInvites = (invites: readonly AccessInvite[]) =>
	invites.filter(
		({ status }) => status !== AccessInviteStatus.Unused && status !== AccessInviteStatus.Used,
	)

/** Revoked devices of nobody (made in the terminal) or of somebody who is gone: the ones the page shows nowhere else */
const looseDevices = (devices: readonly AccessDevice[]) =>
	devices.filter(
		({ status, person }) => status === AccessDeviceStatus.Revoked && (!person || person.removed),
	)

/** What is behind *Show ended*, as lists */
function useEnded() {
	return {
		links: endedLinks(useAccessSignupLinks(true).nodes),
		invites: endedInvites(useAccessInvites(true).nodes),
		devices: looseDevices(useAccessDevices(null, true).nodes),
		passes: oldPasses(useAllAccessDevices().nodes),
	}
}

/** How many things are behind *Show ended* */
export function useEndedCount() {
	const { links, invites, devices, passes } = useEnded()
	return links.length + invites.length + devices.length + passes.length
}

/**
 * What ended: links that were used up, expired or revoked, and revoked devices that belong to
 * nobody the page lists any more (a pass made in the terminal, a person who is gone). Each says
 * what it was and when; a revoked device can be brought back, and a link that made accounts can
 * still turn them off. First, the old passes that still work (nobody's, or a gone person's):
 * nothing else on the page lists them, so each can be removed, or given more tries, from here
 */
export default function EndedList() {
	const { t } = useTranslate()
	const { links, invites, devices, passes } = useEnded()
	const actions = useAccessActions()
	const [revoking, setRevoking] = useState<Revoking | null>(null)

	const remove = async (device: AccessDevice) => {
		try {
			await actions.revokeDevice(device.id)
			toast.success(t(accessKey('attention.leftoversRemoved'), { count: 1 }))
		} catch (error) {
			toast.error(t(accessKey('ended.error')), { description: errorMessage(error) })
		}
	}

	const allowMoreTries = async (device: AccessDevice) => {
		try {
			await actions.resetBudget(device.id)
			toast.success(t(accessKey('person.triesAllowed')))
		} catch (error) {
			toast.error(t(accessKey('ended.error')), { description: errorMessage(error) })
		}
	}

	const restore = async (device: AccessDevice) => {
		try {
			await actions.restoreDevice(device.id)
			toast.success(t(accessKey('ended.restored'), { label: device.label }))
		} catch (error) {
			toast.error(t(accessKey('ended.error')), { description: errorMessage(error) })
		}
	}

	return (
		<div data-testid="access-ended" className="divide-y divide-border/70">
			{passes.length > 0 && (
				<Text size="xs" variant="muted" className="px-4 pt-3 pb-2 font-medium">
					{t(accessKey('ended.oldPasses'))}
				</Text>
			)}
			{passes.map((device) => (
				<div key={device.id} className="gap-3 px-4 py-3 flex flex-wrap items-center">
					<span className="min-w-0 flex-1">
						<span className="gap-2 flex items-center">
							<Text size="sm" className="font-medium">
								{device.label}
							</Text>
							<Badge size="xs" variant="secondary">
								{t(
									accessKey(
										device.person && !device.legacy
											? 'ended.deviceOfRemoved'
											: 'person.kind.terminal',
									),
								)}
							</Badge>
						</span>
						<Text size="xs" variant="muted">
							{t(accessKey('person.used'), {
								when: formatWhen(device.lastSeenAt ?? device.createdAt),
							})}
						</Text>
					</span>
					{!device.person && (
						<Button
							variant="outline"
							size="sm"
							disabled={actions.isPending}
							onClick={() => allowMoreTries(device)}
						>
							{t(accessKey('person.allowMoreTries'))}
						</Button>
					)}
					<Button
						variant="outline"
						size="sm"
						disabled={actions.isPending}
						onClick={() => remove(device)}
					>
						{t(accessKey('person.removeDevice'))}
					</Button>
				</div>
			))}
			{links.map((link) => (
				<div key={link.id} className="gap-3 px-4 py-3 flex items-center">
					<span className="min-w-0 flex-1">
						<Text size="sm" className="font-medium">
							{link.label}
						</Text>
						<Text size="xs" variant="muted">
							{t(accessKey(`ended.status.${link.status}`))} ·{' '}
							{formatWhen(link.revokedAt ?? link.expiresAt)}
						</Text>
					</span>
					{link.people.length > 0 && (
						<DropdownMenu
							align="end"
							trigger={
								<Button variant="ghost" size="icon" aria-label={t(accessKey('people.moreActions'))}>
									<MoreHorizontal className="h-4 w-4" />
								</Button>
							}
							groups={[
								{
									items: [
										{
											label: t(accessKey('ended.turnOffAccounts')),
											onClick: () =>
												setRevoking({
													kind: 'signup',
													id: link.id,
													label: link.label,
													accounts: link.people.length,
													turnOffOnly: true,
												}),
										},
									],
								},
							]}
						/>
					)}
				</div>
			))}
			{invites.map((invite) => (
				<div key={invite.id} className="gap-3 px-4 py-3 flex items-center">
					<span className="min-w-0 flex-1">
						<Text size="sm" className="font-medium">
							{invite.label}
						</Text>
						<Text size="xs" variant="muted">
							{t(accessKey(`ended.status.${invite.status}`))} · {formatWhen(invite.expiresAt)}
							{invite.person && ` · ${invite.person.username}`}
						</Text>
					</span>
				</div>
			))}
			{devices.map((device) => (
				<div key={device.id} className="gap-3 px-4 py-3 flex items-center">
					<span className="min-w-0 flex-1">
						<span className="gap-2 flex items-center">
							<Text size="sm" className="font-medium">
								{device.label}
							</Text>
							<Badge size="xs" variant="secondary">
								{t(accessKey(device.legacy ? 'person.kind.terminal' : 'ended.deviceOfRemoved'))}
							</Badge>
						</span>
						<Text size="xs" variant="muted">
							{t(accessKey('ended.deviceRemoved'))} · {formatWhen(device.revokedAt)}
						</Text>
					</span>
					<Button
						variant="outline"
						size="sm"
						disabled={actions.isPending}
						onClick={() => restore(device)}
					>
						{t(accessKey('person.restore'))}
					</Button>
				</div>
			))}
			<RevokeLinkDialog link={revoking} onClose={() => setRevoking(null)} />
		</div>
	)
}
