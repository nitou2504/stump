import {
	Alert,
	AlertDescription,
	Button,
	CheckBox,
	Input,
	NativeSelect,
	Text,
} from '@stump/components'
import { SignupLinkKind, UserPermission } from '@stump/graphql'
import { FormEvent, useEffect, useMemo, useState } from 'react'
import { toast } from 'sonner'

import { useTranslate } from '@/hooks/useTranslate'

import type { InvitePrefill } from './AttentionSection'
import {
	useAccessActions,
	useAccessOverview,
	useLibraryChoices,
	usePresetDefaults,
} from './graphql'
import { accessKey } from './keys'
import MadeLink, { type MadeLinkValue } from './MadeLink'
import ModalPanel from './ModalPanel'
import { FRIEND_PERMISSIONS, permissionKey } from './permissions'

const errorMessage = (error: unknown) =>
	error instanceof Error && error.message ? error.message : undefined

/** How long a link works, in hours: a day, three days, a week, two weeks, 30 days */
export const LINK_HOURS = [24, 72, 168, 336, 720] as const
/** How many accounts an open link may make */
export const OPEN_USES = [2, 3, 5, 10, 25] as const
/** The devices a person may have */
export const DEVICE_CHOICES = [1, 2, 3, 4, 5, 6, 8, 10] as const

/** What a link gives, in words: shown on the form so the owner always knows what they are handing out */
function PresetSummary({
	permissions,
	libraryNames,
	maxDevices,
	ageLimit,
}: {
	permissions: string[]
	libraryNames: string[]
	maxDevices: number
	ageLimit: string
}) {
	const { t } = useTranslate()
	const can = permissions.length
		? permissions.map((permission) =>
				t(accessKey(`signups.permission.${permissionKey(permission)}`)),
			)
		: [t(accessKey('signups.summary.nothing'))]

	return (
		<div data-testid="access-preset-summary" className="gap-1 flex flex-col">
			<Text size="sm">
				<span className="font-medium">{t(accessKey('signups.summary.can'))}</span> {can.join(', ')}.
			</Text>
			<Text size="sm">
				<span className="font-medium">{t(accessKey('signups.summary.sees'))}</span>{' '}
				{libraryNames.length ? libraryNames.join(', ') : t(accessKey('signups.summary.nothing'))}.{' '}
				{t(accessKey('signups.summary.later'))}
			</Text>
			<Text size="sm">
				{t(accessKey('signups.summary.devices'), { count: maxDevices })}
				{ageLimit.trim() && ` ${t(accessKey('signups.summary.age'), { age: ageLimit.trim() })}`}
			</Text>
		</div>
	)
}

const toggle = <T,>(list: T[], value: T): T[] =>
	list.includes(value) ? list.filter((item) => item !== value) : [...list, value]

type Props = {
	open: boolean
	onClose: () => void
	/** Values to start from (the owner makes another like a link that is nearly used up) */
	prefill?: InvitePrefill | null
}

/** The form and then, once the link is made, the block that shows it once */
function InviteBody({ prefill, onClose }: { prefill?: InvitePrefill | null; onClose: () => void }) {
	const { t } = useTranslate()
	const overview = useAccessOverview()
	const defaults = usePresetDefaults()
	const libraries = useLibraryChoices()
	const actions = useAccessActions()

	const [label, setLabel] = useState(prefill?.label ?? '')
	const [kind, setKind] = useState<SignupLinkKind>(prefill?.kind ?? SignupLinkKind.Single)
	const [hours, setHours] = useState<number>(72)
	const [uses, setUses] = useState<number>(
		prefill?.maxUses && prefill.maxUses > 1 ? prefill.maxUses : 5,
	)
	const [editing, setEditing] = useState(false)
	const [permissions, setPermissions] = useState<string[]>(
		prefill?.permissions.length ? prefill.permissions : defaults.permissions,
	)
	const [libraryIds, setLibraryIds] = useState<string[]>(
		prefill?.libraryIds.length ? prefill.libraryIds : defaults.libraries.map(({ id }) => id),
	)
	const [maxDevices, setMaxDevices] = useState<number>(prefill?.maxDevices ?? defaults.maxDevices)
	const [ageLimit, setAgeLimit] = useState('')
	const [missing, setMissing] = useState(false)
	const [made, setMade] = useState<MadeLinkValue | null>(null)

	// a new prefill (another link nearly used up) starts the form over
	useEffect(() => {
		setMade(null)
	}, [prefill])

	const libraryNames = useMemo(
		() => libraries.filter(({ id }) => libraryIds.includes(id)).map(({ name }) => name),
		[libraries, libraryIds],
	)
	const age = ageLimit.trim() === '' ? null : Number(ageLimit)
	const ageIsValid = age === null || (Number.isInteger(age) && age >= 1 && age <= 120)
	const isOpen = kind === SignupLinkKind.Open

	const canSubmit = overview.configured && libraryIds.length > 0 && ageIsValid && !actions.isPending

	const submit = async (event: FormEvent) => {
		event.preventDefault()
		if (!label.trim()) {
			setMissing(true)
			return
		}
		if (!canSubmit) return
		try {
			const url = await actions.createSignupLink({
				label: label.trim(),
				kind,
				validForHours: hours,
				maxUses: isOpen ? uses : 1,
				preset: {
					permissions: permissions as UserPermission[],
					libraryIds,
					ageRestriction: age === null ? null : { age, restrictOnUnset: false },
					maxDevices,
				},
			})
			setMade({ url, label: label.trim() })
		} catch (error) {
			toast.error(t(accessKey('invite.error')), { description: errorMessage(error) })
		}
	}

	// the server refuses new accounts while public sign-in is off: say so, and don't offer a form
	if (!overview.publicSignin) {
		return (
			<div className="gap-4 flex flex-col" data-testid="access-signup-off">
				<Alert variant="warning">
					<AlertDescription>{t(accessKey('invite.signinOff'))}</AlertDescription>
				</Alert>
				<div>
					<Button variant="outline" onClick={onClose}>
						{t(accessKey('invite.done'))}
					</Button>
				</div>
			</div>
		)
	}

	if (made) {
		return (
			<MadeLink
				made={made}
				onDone={() => {
					// the address is forgotten with the block
					setMade(null)
					setLabel('')
					onClose()
				}}
			/>
		)
	}

	return (
		<form onSubmit={submit} className="gap-5 flex flex-col" data-testid="access-invite-form">
			<Input
				id="access-invite-who"
				label={t(accessKey('invite.who'))}
				description={t(accessKey('invite.whoHelp'))}
				placeholder={t(accessKey('invite.whoPlaceholder'), { name: 'Rosa' })}
				value={label}
				onChange={(event) => {
					setLabel(event.target.value)
					setMissing(false)
				}}
				maxLength={60}
				fullWidth
				errorMessage={missing ? t(accessKey('invite.whoMissing')) : undefined}
			/>

			<div className="gap-1.5 flex flex-col">
				<span className="text-sm font-medium">{t(accessKey('invite.linkFor'))}</span>
				<div role="group" aria-label={t(accessKey('invite.linkFor'))} className="gap-2 flex">
					<Button
						type="button"
						size="sm"
						variant={isOpen ? 'outline' : 'default'}
						aria-pressed={!isOpen}
						onClick={() => setKind(SignupLinkKind.Single)}
					>
						{t(accessKey('invite.one'))}
					</Button>
					<Button
						type="button"
						size="sm"
						variant={isOpen ? 'default' : 'outline'}
						aria-pressed={isOpen}
						onClick={() => setKind(SignupLinkKind.Open)}
					>
						{t(accessKey('invite.several'))}
					</Button>
				</div>
			</div>

			<div className="gap-4 sm:grid-cols-2 grid grid-cols-1">
				{isOpen && (
					<div className="gap-1.5 flex flex-col">
						<label htmlFor="access-invite-uses" className="text-sm font-medium">
							{t(accessKey('invite.howMany'))}
						</label>
						<NativeSelect
							id="access-invite-uses"
							value={uses}
							onChange={(event) => setUses(Number(event.target.value))}
							options={OPEN_USES.map((value) => ({ label: String(value), value }))}
						/>
					</div>
				)}
				<div className="gap-1.5 flex flex-col">
					<label htmlFor="access-invite-hours" className="text-sm font-medium">
						{t(accessKey('invite.worksFor'))}
					</label>
					<NativeSelect
						id="access-invite-hours"
						value={hours}
						onChange={(event) => setHours(Number(event.target.value))}
						options={LINK_HOURS.map((value) => ({
							label: t(accessKey(`invite.validity.h${value}`)),
							value,
						}))}
					/>
				</div>
			</div>

			<div className="gap-2 p-3 flex flex-col rounded-lg bg-muted/40">
				<Text size="sm" className="font-medium">
					{t(accessKey('signups.form.whatTheyGet'))}
				</Text>
				<PresetSummary
					permissions={permissions}
					libraryNames={libraryNames}
					maxDevices={maxDevices}
					ageLimit={ageLimit}
				/>
				<div>
					<Button
						type="button"
						size="sm"
						variant="ghost"
						aria-expanded={editing}
						onClick={() => setEditing((open) => !open)}
					>
						{t(accessKey(editing ? 'signups.form.done' : 'signups.form.change'))}
					</Button>
				</div>

				{editing && (
					<div data-testid="access-preset-editor" className="gap-4 pt-2 flex flex-col">
						<fieldset className="gap-2 flex flex-col">
							<legend className="text-sm font-medium">
								{t(accessKey('signups.editor.permissions'))}
							</legend>
							{FRIEND_PERMISSIONS.map((permission) => (
								<CheckBox
									key={permission}
									id={`access-signup-${permission}`}
									label={t(accessKey(`signups.permission.${permissionKey(permission)}`))}
									checked={permissions.includes(permission)}
									onCheckedChange={() => setPermissions((current) => toggle(current, permission))}
								/>
							))}
							<Text size="xs" variant="muted">
								{t(accessKey('signups.editor.notOffered'))}
							</Text>
						</fieldset>

						<fieldset className="gap-2 flex flex-col">
							<legend className="text-sm font-medium">
								{t(accessKey('signups.editor.libraries'))}
							</legend>
							{libraries.map(({ id, name }) => (
								<CheckBox
									key={id}
									id={`access-signup-library-${id}`}
									label={name}
									checked={libraryIds.includes(id)}
									onCheckedChange={() => setLibraryIds((current) => toggle(current, id))}
								/>
							))}
							{libraryIds.length === 0 && (
								<Text size="sm" variant="danger">
									{t(accessKey('signups.editor.noLibraries'))}
								</Text>
							)}
						</fieldset>

						<div className="gap-4 sm:grid-cols-2 grid grid-cols-1">
							<div className="gap-1.5 flex flex-col">
								<label htmlFor="access-signup-devices" className="text-sm font-medium">
									{t(accessKey('signups.editor.devices'))}
								</label>
								<NativeSelect
									id="access-signup-devices"
									value={maxDevices}
									onChange={(event) => setMaxDevices(Number(event.target.value))}
									options={DEVICE_CHOICES.map((value) => ({ label: String(value), value }))}
								/>
							</div>
							<Input
								id="access-signup-age"
								label={t(accessKey('signups.editor.age'))}
								placeholder={t(accessKey('signups.editor.agePlaceholder'))}
								inputMode="numeric"
								value={ageLimit}
								onChange={(event) => setAgeLimit(event.target.value.replace(/\D/g, ''))}
								maxLength={3}
								fullWidth
							/>
						</div>
						{!ageIsValid && (
							<Text size="sm" variant="danger">
								{t(accessKey('signups.editor.ageInvalid'))}
							</Text>
						)}
					</div>
				)}
			</div>

			{isOpen && permissions.includes(UserPermission.AddBooks) && (
				<Alert variant="warning" data-testid="access-open-addbooks">
					<AlertDescription>{t(accessKey('signups.openAddBooks'))}</AlertDescription>
				</Alert>
			)}

			<div className="gap-2 flex">
				<Button type="button" variant="outline" onClick={onClose}>
					{t(accessKey('invite.cancel'))}
				</Button>
				<Button type="submit" disabled={!canSubmit} isLoading={actions.isPending}>
					{t(accessKey('invite.create'))}
				</Button>
			</div>
		</form>
	)
}

/**
 * *Invite someone*: the one thing the owner does here. One question to start (who it is for), one
 * choice (one person or several), how long it works, and what the new account gets (the default
 * is shown, and *Change* opens the editor). The link that comes back is shown once, with *Copy*
 */
export default function InviteDialog({ open, onClose, prefill }: Props) {
	const { t } = useTranslate()

	return (
		<ModalPanel
			open={open}
			onClose={onClose}
			title={t(accessKey('invite.title'))}
			testId="access-invite"
		>
			{open && <InviteBody prefill={prefill} onClose={onClose} />}
		</ModalPanel>
	)
}
