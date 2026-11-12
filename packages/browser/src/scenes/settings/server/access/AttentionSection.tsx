import {
	Button,
	ButtonOrLink,
	CheckBox,
	ConfirmationModal,
	IconButton,
	Text,
	ToolTip,
} from '@stump/components'
import { AccessDeviceStatus, AccessWarningSeverity, SignupLinkKind } from '@stump/graphql'
import { AlertTriangle, CheckCircle2, ShieldAlert, X } from 'lucide-react'
import { useState } from 'react'
import { toast } from 'sonner'

import { useTranslate } from '@/hooks/useTranslate'
import { usePaths } from '@/paths'

import {
	type AttentionAction,
	describeWarning,
	nextLimit,
	SHOWN_AT_FIRST,
	sortWarnings,
} from './attention'
import { formatAgo, formatWhen } from './format'
import {
	type AccessDevice,
	type AccessPersonRow,
	type AccessWarning,
	useAccessActions,
	useAccessPeople,
	useAccessSignupLinks,
	useAccessWarnings,
	useAllAccessDevices,
} from './graphql'
import { accessKey } from './keys'
import { inSentence } from './permissions'

const errorMessage = (error: unknown) =>
	error instanceof Error && error.message ? error.message : undefined

/** A device that has not been used for this long no longer counts and is a leftover (the server's number) */
export const DORMANT_DAYS = 60

/** What a new link starts from when the owner makes another like one that is nearly used up */
export type InvitePrefill = {
	label: string
	kind: SignupLinkKind
	maxUses: number
	permissions: string[]
	libraryIds: string[]
	maxDevices: number
}

type Props = {
	/** Opens a person's sheet (their devices, password, libraries) */
	onOpenPerson: (personId: string) => void
	/** Opens the dialog that makes an invite, with these values filled in */
	onInvite: (prefill: InvitePrefill) => void
	/** Opens the history on the failed sign-ins */
	onSeeAttempts: () => void
	/** Opens the dialog that lets somebody in from the internet (or fixes what keeps them out) */
	onLetIn: (row: AccessPersonRow) => void
	/** Opens the dialog that sets somebody's password */
	onSetPassword: (person: { id: string; username: string }) => void
}

/** Devices that still work but are leftovers: made in the terminal, of somebody who is gone, or not used for two months */
export function leftoverDevices(
	devices: readonly AccessDevice[],
	now = Date.now(),
): AccessDevice[] {
	const cutoff = now - DORMANT_DAYS * 24 * 60 * 60 * 1000
	return devices.filter((device) => {
		if (device.status !== AccessDeviceStatus.Active) return false
		const seen = new Date(device.lastSeenAt ?? device.createdAt).getTime()
		return device.legacy || !device.person || device.person.removed || seen <= cutoff
	})
}

/**
 * *Needs your attention*: everything the owner should look at, in one list at the top of the
 * page, one line each with its one button and a way to dismiss it. *Clear all* marks everything
 * shown as seen; nothing is deleted (the history keeps it all) and an item comes back when
 * something new, or something worse, happens. When there is nothing, one calm line says so.
 */
export default function AttentionSection({
	onOpenPerson,
	onInvite,
	onSeeAttempts,
	onLetIn,
	onSetPassword,
}: Props) {
	const { t, translate } = useTranslate()
	const paths = usePaths()
	const warnings = sortWarnings(useAccessWarnings())
	const people = useAccessPeople()
	const openLinks = useAccessSignupLinks(false).nodes
	const actions = useAccessActions()

	const [all, setAll] = useState(false)
	const [confirm, setConfirm] = useState<
		| { kind: 'signOutAll'; warning: AccessWarning }
		| { kind: 'revokeLink'; warning: AccessWarning }
		| { kind: 'leftovers' }
		| null
	>(null)

	const shown = all ? warnings : warnings.slice(0, SHOWN_AT_FIRST)

	const undoable = (message: string, ids: string[]) =>
		toast(message, {
			action: {
				label: t(accessKey('attention.undo')),
				onClick: () => {
					void actions.restoreWarnings(ids)
				},
			},
		})

	const dismiss = async (warning: AccessWarning) => {
		try {
			await actions.dismissWarnings([warning.id])
			undoable(t(accessKey('attention.dismissed')), [warning.id])
		} catch (error) {
			toast.error(t(accessKey('attention.error')), { description: errorMessage(error) })
		}
	}

	const clearAll = async () => {
		const ids = warnings.map(({ id }) => id)
		try {
			// the ones that were on the screen: something that came in after the last refresh was
			// never shown, and is not cleared by this click
			const done = await actions.dismissWarnings(ids)
			undoable(t(accessKey('attention.cleared'), { count: done }), ids)
		} catch (error) {
			toast.error(t(accessKey('attention.error')), { description: errorMessage(error) })
		}
	}

	/** The link is nearly used up: make another like it, with the same note and what it gives */
	const openNewLink = (warning: AccessWarning) => {
		const link = openLinks.find(({ id }) => id === warning.linkId)
		onInvite({
			label: link?.label ?? warning.detail ?? '',
			kind: link?.kind ?? SignupLinkKind.Open,
			maxUses: link?.maxUses ?? 5,
			permissions: link?.preset?.permissions ?? [],
			libraryIds: link?.preset?.libraries.map(({ id }) => id) ?? [],
			maxDevices: link?.preset?.maxDevices ?? 3,
		})
	}

	const rowOf = (warning: AccessWarning) =>
		people.find(({ person }) => person.id === warning.person?.id) ?? null

	const run = async (action: AttentionAction, warning: AccessWarning) => {
		const person = warning.person
		const row = rowOf(warning)
		try {
			switch (action) {
				case 'raiseLimit': {
					const next = nextLimit(people, person?.id)
					if (!person || !row || next === null) return
					await actions.setPersonAccess({ personId: person.id, maxDevices: next })
					toast(t(accessKey('attention.raised'), { name: person.username, count: next }), {
						action: {
							label: t(accessKey('attention.undo')),
							onClick: () => {
								void actions.setPersonAccess({ personId: person.id, maxDevices: row.maxDevices })
							},
						},
					})
					return
				}
				case 'seeDevices':
					if (person) onOpenPerson(person.id)
					return
				case 'signOutAll':
					setConfirm({ kind: 'signOutAll', warning })
					return
				case 'setPassword':
					if (person) onSetPassword(person)
					return
				case 'seeAttempts':
					onSeeAttempts()
					return
				case 'letIn':
				case 'fix':
					if (row) onLetIn(row)
					return
				case 'newLink':
					openNewLink(warning)
					return
				case 'revokeLink':
					setConfirm({ kind: 'revokeLink', warning })
					return
				case 'unblock':
					if (person) {
						await actions.unblock(person.id)
						toast.success(t(accessKey('attention.unblocked'), { name: person.username }))
					}
					return
				case 'removeLeftovers':
					setConfirm({ kind: 'leftovers' })
					return
				case 'review':
					return
			}
		} catch (error) {
			toast.error(t(accessKey('attention.error')), { description: errorMessage(error) })
		}
	}

	return (
		<section
			id="access-attention"
			data-testid="access-attention"
			aria-labelledby="access-attention-title"
			className="gap-3 flex flex-col"
		>
			{warnings.length === 0 ? (
				<div data-testid="access-all-good" className="gap-2 py-1 flex items-center">
					<CheckCircle2 aria-hidden className="h-4 w-4 text-green-500 shrink-0" />
					<Text id="access-attention-title" size="sm" variant="muted">
						{t(accessKey('attention.allGood'))}
					</Text>
				</div>
			) : (
				<>
					<div className="gap-3 flex items-center justify-between">
						<Text id="access-attention-title" className="font-medium">
							{t(accessKey('attention.title'), { count: warnings.length })}
						</Text>
						<Button
							size="sm"
							variant="ghost"
							onClick={clearAll}
							disabled={actions.isPending}
							data-testid="access-clear-all"
						>
							{t(accessKey('attention.clearAll'))}
						</Button>
					</div>
					<ul className="divide-y divide-border rounded-lg bg-card ring-1 ring-foreground/10">
						{shown.map((warning) => (
							<Item
								key={warning.id}
								warning={warning}
								people={people}
								busy={actions.isPending}
								onRun={run}
								onDismiss={dismiss}
								reviewHref={paths.bookApprovals()}
							/>
						))}
					</ul>
					{warnings.length > SHOWN_AT_FIRST && (
						<div>
							<Button size="sm" variant="ghost" onClick={() => setAll((open) => !open)}>
								{all
									? t(accessKey('attention.showFewer'))
									: t(accessKey('attention.showAll'), { count: warnings.length })}
							</Button>
						</div>
					)}
				</>
			)}

			<ConfirmationModal
				isOpen={confirm?.kind === 'signOutAll'}
				title={t(accessKey('person.signOutAll'))}
				description={t(accessKey('person.signOutAllConfirm'), {
					name: confirm?.kind === 'signOutAll' ? (confirm.warning.person?.username ?? '') : '',
				})}
				confirmText={t(accessKey('person.signOutAll'))}
				cancelText={translate('shared.common.cancel')}
				confirmIsLoading={actions.isPending}
				onClose={() => setConfirm(null)}
				onConfirm={async () => {
					if (confirm?.kind !== 'signOutAll' || !confirm.warning.person) return
					try {
						await actions.signOutEverywhere(confirm.warning.person.id)
						setConfirm(null)
					} catch (error) {
						toast.error(t(accessKey('attention.error')), { description: errorMessage(error) })
					}
				}}
			/>
			<ConfirmationModal
				isOpen={confirm?.kind === 'revokeLink'}
				title={t(accessKey('attention.actions.revokeLink'))}
				description={t(accessKey('attention.revokeLinkConfirm'), {
					note: confirm?.kind === 'revokeLink' ? (confirm.warning.detail ?? '') : '',
				})}
				confirmText={t(accessKey('attention.actions.revokeLink'))}
				confirmVariant="destructive"
				cancelText={translate('shared.common.cancel')}
				confirmIsLoading={actions.isPending}
				onClose={() => setConfirm(null)}
				onConfirm={async () => {
					if (confirm?.kind !== 'revokeLink' || !confirm.warning.linkId) return
					try {
						await actions.revokeSignupLink(confirm.warning.linkId, false)
						setConfirm(null)
					} catch (error) {
						toast.error(t(accessKey('attention.error')), { description: errorMessage(error) })
					}
				}}
			/>
			{confirm?.kind === 'leftovers' && <LeftoversConfirm onClose={() => setConfirm(null)} />}
		</section>
	)
}

/** One line of the list: what it says, its one button and the way to dismiss it */
function Item({
	warning,
	people,
	busy,
	onRun,
	onDismiss,
	reviewHref,
}: {
	warning: AccessWarning
	people: readonly AccessPersonRow[]
	busy: boolean
	onRun: (action: AttentionAction, warning: AccessWarning) => void
	onDismiss: (warning: AccessWarning) => void
	reviewHref: string
}) {
	const { t } = useTranslate()
	const isAlert = warning.severity === AccessWarningSeverity.Alert
	const Icon = isAlert ? ShieldAlert : AlertTriangle
	const { key, action } = describeWarning(warning)
	const row = people.find(({ person }) => person.id === warning.person?.id)

	// at the most devices there are, there is nothing to raise the limit to: look at the devices instead
	const effective: AttentionAction =
		action === 'raiseLimit' && row && nextLimit(people, warning.person?.id) === null
			? 'seeDevices'
			: action

	const sentence = t(accessKey(`attention.kinds.${key}`), {
		name: warning.person?.username ?? '',
		count: warning.count,
		limit: row?.maxDevices ?? 3,
		note: warning.detail ?? '',
		extras: (warning.detail ?? '')
			.split(',')
			.map((word) => inSentence(t(`userPermissions.${word.trim()}.label`)))
			.join(', '),
	})

	return (
		<li
			data-testid={`access-warning-${warning.id}`}
			data-severity={warning.severity}
			className="gap-x-3 gap-y-2 px-4 py-3 sm:flex-row sm:items-center flex flex-col justify-between"
		>
			<div className="gap-3 min-w-0 flex items-start">
				<Icon
					aria-hidden
					className={
						isAlert
							? 'mt-0.5 h-4 w-4 shrink-0 text-destructive'
							: 'mt-0.5 h-4 w-4 text-amber-500 shrink-0'
					}
				/>
				<div className="min-w-0">
					<Text size="sm">{sentence}</Text>
					<Text size="xs" variant="muted" title={formatWhen(warning.lastAt)}>
						{formatAgo(warning.lastAt)}
					</Text>
				</div>
			</div>
			<div className="gap-2 flex shrink-0 items-center justify-end">
				{effective === 'review' ? (
					<ButtonOrLink size="sm" variant="outline" href={reviewHref}>
						{t(accessKey(`attention.actions.${effective}`))}
					</ButtonOrLink>
				) : (
					<Button
						size="sm"
						variant="outline"
						disabled={busy}
						onClick={() => onRun(effective, warning)}
					>
						{t(accessKey(`attention.actions.${effective}`))}
					</Button>
				)}
				<ToolTip content={t(accessKey('attention.dismiss'))}>
					<IconButton
						variant="ghost"
						size="sm"
						aria-label={t(accessKey('attention.dismiss'))}
						disabled={busy}
						onClick={() => onDismiss(warning)}
					>
						<X className="h-4 w-4" />
					</IconButton>
				</ToolTip>
			</div>
		</li>
	)
}

/** Lists the old devices that still work and removes them when the owner says so */
function LeftoversConfirm({ onClose }: { onClose: () => void }) {
	const { t, translate } = useTranslate()
	const devices = leftoverDevices(useAllAccessDevices().nodes)
	const actions = useAccessActions()
	// all ticked at first; the owner unticks what should keep working
	const [kept, setKept] = useState<readonly string[]>([])
	const chosen = devices.filter(({ id }) => !kept.includes(id))

	return (
		<ConfirmationModal
			isOpen
			title={t(accessKey('attention.actions.removeLeftovers'))}
			description={t(accessKey('attention.leftoversConfirm'), { count: chosen.length })}
			confirmText={t(accessKey('attention.actions.removeLeftovers'))}
			confirmVariant="destructive"
			confirmDisabled={chosen.length === 0}
			cancelText={translate('shared.common.cancel')}
			confirmIsLoading={actions.isPending}
			onClose={onClose}
			onConfirm={async () => {
				try {
					for (const device of chosen) await actions.revokeDevice(device.id)
					toast.success(t(accessKey('attention.leftoversRemoved'), { count: chosen.length }))
					onClose()
				} catch (error) {
					toast.error(t(accessKey('attention.error')), { description: errorMessage(error) })
				}
			}}
		>
			<ul className="gap-2 text-sm flex flex-col" data-testid="access-leftovers">
				{devices.map((device) => (
					<li key={device.id}>
						<CheckBox
							id={`access-leftover-${device.id}`}
							checked={!kept.includes(device.id)}
							onCheckedChange={() =>
								setKept(
									kept.includes(device.id)
										? kept.filter((id) => id !== device.id)
										: [...kept, device.id],
								)
							}
							label={device.label}
							description={`${device.person?.username ?? t(accessKey('person.kind.terminal'))} · ${formatWhen(device.lastSeenAt ?? device.createdAt)}`}
						/>
					</li>
				))}
			</ul>
		</ConfirmationModal>
	)
}
