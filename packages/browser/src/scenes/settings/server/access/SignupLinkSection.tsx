import {
	Alert,
	AlertDescription,
	Badge,
	Button,
	Card,
	CheckBox,
	cn,
	Dialog,
	Input,
	NativeSelect,
	Switch,
	Text,
} from '@stump/components'
import { SignupLinkKind, SignupLinkStatus, UserPermission } from '@stump/graphql'
import { Check, Copy, Share2 } from 'lucide-react'
import { FormEvent, Suspense, useMemo, useState, useTransition } from 'react'
import { toast } from 'sonner'

import { useTranslate } from '@/hooks/useTranslate'

import { copyText } from './copy'
import { formatWhen } from './format'
import {
	type AccessSignupLink,
	useAccessActions,
	useAccessOverview,
	useAccessSignupLinks,
	useLibraryChoices,
	usePresetDefaults,
} from './graphql'
import { accessKey } from './keys'
import Pager from './Pager'
import PersonLabel from './PersonLabel'
import QrCode from './QrCode'
import Section, { Empty, Row, RowList } from './Section'

const errorMessage = (error: unknown) =>
	error instanceof Error && error.message ? error.message : undefined

/**
 * What a link can give: the permissions a friend may have, and no others. The server refuses
 * anything else (Upload files and Scan library included), so nothing else is offered
 */
export const FRIEND_PERMISSIONS = [
	UserPermission.ChangePassword,
	UserPermission.ChangeUsername,
	UserPermission.ChangeAvatar,
	UserPermission.DownloadFile,
	UserPermission.AddBooks,
	UserPermission.AccessSmartList,
] as const

/** How long a link works, in hours: a day, three days, a week, two weeks, 30 days */
export const LINK_HOURS = [24, 72, 168, 336, 720] as const
/** How many accounts an open link may make */
export const OPEN_USES = [2, 3, 5, 10, 25] as const
/** The devices a person may have */
export const DEVICE_CHOICES = [1, 2, 3, 4, 5, 6, 8, 10] as const

/** The permission's name in the locale files: `ADD_BOOKS` is `addBooks` */
const permissionKey = (permission: UserPermission) =>
	permission
		.toLowerCase()
		.replace(/_([a-z])/g, (_, letter: string) => letter.toUpperCase())

/** What the owner is shown right after making a link: the one and only copy of its address */
type MadeLink = { url: string; label: string }

/**
 * A link that was just made, with buttons to copy and share it and a QR code for a phone to scan.
 * It is shown here and nowhere else: Stump keeps a hash of the token and can't show the link
 * again, so a lost one means making another
 */
function MadeLinkBox({ made, onDismiss }: { made: MadeLink; onDismiss: () => void }) {
	const { t } = useTranslate()
	const [copied, setCopied] = useState(false)
	const canShare = typeof navigator !== 'undefined' && typeof navigator.share === 'function'

	const copy = async () => {
		const done = await copyText(made.url)
		setCopied(done)
		if (done) {
			toast.success(t(accessKey('signups.created.copied')))
		} else {
			toast.error(t(accessKey('signups.created.copyFailed')))
		}
	}

	const share = async () => {
		try {
			await navigator.share({ title: made.label, url: made.url })
		} catch {
			// closed without sharing, or refused: nothing to say
		}
	}

	return (
		<Card data-testid="access-made-signup" className="gap-4 p-4 flex flex-col" role="status">
			<Text className="font-medium">
				{t(accessKey('signups.created.title'), { label: made.label })}
			</Text>
			<div className="gap-2 sm:flex-row flex flex-col">
				<Input
					aria-label={t(accessKey('signups.created.linkLabel'))}
					value={made.url}
					readOnly
					fullWidth
					onFocus={(event) => event.currentTarget.select()}
				/>
				<div className="gap-2 flex shrink-0">
					<Button onClick={copy}>
						{copied ? <Check className="h-4 w-4" /> : <Copy className="h-4 w-4" />}
						{t(accessKey('signups.created.copy'))}
					</Button>
					{canShare && (
						<Button variant="outline" onClick={share}>
							<Share2 className="h-4 w-4" />
							{t(accessKey('signups.created.share'))}
						</Button>
					)}
				</div>
			</div>
			<QrCode value={made.url} />
			<Text size="sm" variant="muted">
				{t(accessKey('signups.created.once'))}
			</Text>
			<div>
				<Button size="sm" variant="ghost" onClick={onDismiss}>
					{t(accessKey('signups.created.dismiss'))}
				</Button>
			</div>
		</Card>
	)
}

/** What a link gives, in words: shown on the form so the owner always knows what they are handing out */
function PresetSummary({
	permissions,
	libraryNames,
	maxDevices,
	ageLimit,
}: {
	permissions: UserPermission[]
	libraryNames: string[]
	maxDevices: number
	ageLimit: string
}) {
	const { t } = useTranslate()
	const can = permissions.length
		? permissions.map((permission) => t(accessKey(`signups.permission.${permissionKey(permission)}`)))
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

function SignupForm({ onMade }: { onMade: (made: MadeLink) => void }) {
	const { t } = useTranslate()
	const overview = useAccessOverview()
	const defaults = usePresetDefaults()
	const libraries = useLibraryChoices()
	const actions = useAccessActions()

	const [label, setLabel] = useState('')
	const [kind, setKind] = useState<SignupLinkKind>(SignupLinkKind.Single)
	const [hours, setHours] = useState<number>(72)
	const [uses, setUses] = useState<number>(5)
	const [editing, setEditing] = useState(false)
	const [permissions, setPermissions] = useState<UserPermission[]>(defaults.permissions)
	const [libraryIds, setLibraryIds] = useState<string[]>(defaults.libraries.map(({ id }) => id))
	const [maxDevices, setMaxDevices] = useState<number>(defaults.maxDevices)
	const [ageLimit, setAgeLimit] = useState('')

	const libraryNames = useMemo(
		() => libraries.filter(({ id }) => libraryIds.includes(id)).map(({ name }) => name),
		[libraries, libraryIds],
	)
	const age = ageLimit.trim() === '' ? null : Number(ageLimit)
	const ageIsValid = age === null || (Number.isInteger(age) && age >= 1 && age <= 120)

	const isOpen = kind === SignupLinkKind.Open
	const canSubmit =
		overview.configured &&
		overview.publicSignin &&
		!!label.trim() &&
		libraryIds.length > 0 &&
		ageIsValid &&
		!actions.isPending

	const toggle = <T,>(list: T[], value: T): T[] =>
		list.includes(value) ? list.filter((item) => item !== value) : [...list, value]

	const submit = async (event: FormEvent) => {
		event.preventDefault()
		if (!canSubmit) return
		try {
			const url = await actions.createSignupLink({
				label: label.trim(),
				kind,
				validForHours: hours,
				maxUses: isOpen ? uses : 1,
				preset: {
					permissions,
					libraryIds,
					ageRestriction: age === null ? null : { age, restrictOnUnset: false },
					maxDevices,
				},
			})
			onMade({ url, label: label.trim() })
			setLabel('')
		} catch (error) {
			toast.error(t(accessKey('signups.errors.create')), { description: errorMessage(error) })
		}
	}

	return (
		<Card className="gap-4 p-4 flex flex-col">
			{!overview.publicSignin && (
				<Alert variant="warning" data-testid="access-signup-off">
					<AlertDescription>{t(accessKey('signups.off'))}</AlertDescription>
				</Alert>
			)}

			<form onSubmit={submit} className="gap-4 flex flex-col">
				<div className="gap-4 md:grid-cols-2 xl:grid-cols-4 grid grid-cols-1 items-end">
					<Input
						id="access-signup-label"
						label={t(accessKey('signups.form.label'))}
						placeholder={t(accessKey('signups.form.labelPlaceholder'))}
						value={label}
						onChange={(event) => setLabel(event.target.value)}
						maxLength={60}
						fullWidth
					/>
					<div className="gap-1.5 flex flex-col">
						<label htmlFor="access-signup-kind" className="text-sm font-medium">
							{t(accessKey('signups.form.kind'))}
						</label>
						<NativeSelect
							id="access-signup-kind"
							value={kind}
							onChange={(event) => setKind(event.target.value as SignupLinkKind)}
							options={[
								{ label: t(accessKey('signups.kind.single')), value: SignupLinkKind.Single },
								{ label: t(accessKey('signups.kind.open')), value: SignupLinkKind.Open },
							]}
						/>
					</div>
					<div className="gap-1.5 flex flex-col">
						<label htmlFor="access-signup-hours" className="text-sm font-medium">
							{t(accessKey('signups.form.validFor'))}
						</label>
						<NativeSelect
							id="access-signup-hours"
							value={hours}
							onChange={(event) => setHours(Number(event.target.value))}
							options={LINK_HOURS.map((value) => ({
								label: t(accessKey(`signups.validity.h${value}`)),
								value,
							}))}
						/>
					</div>
					<div className="gap-1.5 flex flex-col">
						<label htmlFor="access-signup-uses" className="text-sm font-medium">
							{t(accessKey('signups.form.uses'))}
						</label>
						{isOpen ? (
							<NativeSelect
								id="access-signup-uses"
								value={uses}
								onChange={(event) => setUses(Number(event.target.value))}
								options={OPEN_USES.map((value) => ({ label: String(value), value }))}
							/>
						) : (
							<NativeSelect
								id="access-signup-uses"
								value={1}
								disabled
								onChange={() => undefined}
								options={[{ label: '1', value: 1 }]}
							/>
						)}
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

				<div>
					<Button type="submit" disabled={!canSubmit} isLoading={actions.isPending}>
						{t(accessKey('signups.form.create'))}
					</Button>
				</div>
			</form>
		</Card>
	)
}

const STATUS_BADGE: Record<SignupLinkStatus, 'success' | 'secondary' | 'warning'> = {
	[SignupLinkStatus.Active]: 'success',
	[SignupLinkStatus.UsedUp]: 'secondary',
	[SignupLinkStatus.Expired]: 'secondary',
	[SignupLinkStatus.Revoked]: 'warning',
}

const STATUS_KEY: Record<SignupLinkStatus, string> = {
	[SignupLinkStatus.Active]: 'active',
	[SignupLinkStatus.UsedUp]: 'usedUp',
	[SignupLinkStatus.Expired]: 'expired',
	[SignupLinkStatus.Revoked]: 'revoked',
}

/** Taking a link back, and optionally turning off the accounts it made */
function RevokeDialog({ link, onClose }: { link: AccessSignupLink | null; onClose: () => void }) {
	const { t, translate } = useTranslate()
	const actions = useAccessActions()
	const [disableAccounts, setDisableAccounts] = useState(false)

	const confirm = async () => {
		if (!link) return
		try {
			const turnedOff = await actions.revokeSignupLink(link.id, disableAccounts)
			toast.success(t(accessKey('signups.revoked'), { label: link.label }), {
				description: disableAccounts
					? t(accessKey('signups.revokedAccounts'), { count: turnedOff })
					: undefined,
			})
			onClose()
		} catch (error) {
			toast.error(t(accessKey('signups.errors.revoke')), { description: errorMessage(error) })
		}
	}

	return (
		<Dialog
			open={!!link}
			onOpenChange={(open) => {
				if (!open) {
					setDisableAccounts(false)
					onClose()
				}
			}}
		>
			<Dialog.Content size="sm">
				<Dialog.Header>
					<Dialog.Title>{t(accessKey('signups.confirm.title'))}</Dialog.Title>
					<Dialog.Close onClick={onClose} />
				</Dialog.Header>
				<div className="gap-4 flex flex-col">
					<Text size="sm">
						{t(accessKey('signups.confirm.description'), { label: link?.label ?? '' })}
					</Text>
					{!!link?.people.length && (
						<div className="gap-1 flex flex-col">
							<Switch
								id="access-revoke-accounts"
								label={t(accessKey('signups.confirm.alsoAccounts'), { count: link.people.length })}
								checked={disableAccounts}
								onCheckedChange={setDisableAccounts}
							/>
							<Text size="sm" variant="muted">
								{t(accessKey('signups.confirm.alsoAccountsHint'))}
							</Text>
						</div>
					)}
				</div>
				<Dialog.Footer>
					<Button variant="outline" onClick={onClose}>
						{translate('shared.common.cancel')}
					</Button>
					<Button variant="destructive" onClick={confirm} isLoading={actions.isPending}>
						{t(accessKey('signups.confirm.revoke'))}
					</Button>
				</Dialog.Footer>
			</Dialog.Content>
		</Dialog>
	)
}

function SignupList() {
	const { t } = useTranslate()
	const [page, setPage] = useState(1)
	// the page that is shown stays until the next one is ready
	const [isPaging, startTransition] = useTransition()
	const { nodes, pageInfo } = useAccessSignupLinks(page)
	const [revoking, setRevoking] = useState<AccessSignupLink | null>(null)

	return (
		<div className={cn('gap-y-3 flex flex-col', { 'opacity-60': isPaging })} aria-busy={isPaging}>
			<RowList data-testid="access-signup-list">
				{nodes.length === 0 && <Empty>{t(accessKey('signups.empty'))}</Empty>}
				{nodes.map((link) => (
					<Row key={link.id} data-testid={`access-signup-${link.id}`}>
						<div className="min-w-0">
							<div className="gap-2 flex flex-wrap items-center">
								<Text size="sm" className="font-medium">
									{link.label}
								</Text>
								<Badge size="xs" variant="secondary">
									{t(
										accessKey(
											link.kind === SignupLinkKind.Open ? 'signups.kind.open' : 'signups.kind.single',
										),
									)}
								</Badge>
								<Badge size="xs" variant={STATUS_BADGE[link.status]}>
									{t(accessKey(`signups.status.${STATUS_KEY[link.status]}`))}
								</Badge>
							</div>
							<Text size="xs" variant="muted" className="mt-1">
								{t(accessKey('signups.detail'), {
									created: formatWhen(link.createdAt),
									expires: formatWhen(link.expiresAt),
									uses: link.uses,
									max: link.maxUses,
								})}
							</Text>
							{link.people.length > 0 && (
								<div className="mt-1 gap-x-3 flex flex-wrap items-center">
									<Text size="xs" variant="muted">
										{t(accessKey('signups.madeAccounts'))}
									</Text>
									{link.people.map((person) => (
										<PersonLabel key={person.id} person={person} />
									))}
								</div>
							)}
						</div>
						{link.status === SignupLinkStatus.Active ? (
							<Button size="sm" variant="outline" onClick={() => setRevoking(link)}>
								{t(accessKey('signups.revoke'))}
							</Button>
						) : (
							link.people.length > 0 && (
								<Button size="sm" variant="ghost" onClick={() => setRevoking(link)}>
									{t(accessKey('signups.turnOffAccounts'))}
								</Button>
							)
						)}
					</Row>
				))}
			</RowList>
			<Pager
				page={page}
				totalPages={pageInfo.totalPages}
				onChange={(next) => startTransition(() => setPage(next))}
			/>
			<RevokeDialog link={revoking} onClose={() => setRevoking(null)} />
		</div>
	)
}

/**
 * Signup links: the way a friend gets an account, with no work for the owner but the link. It is
 * for one person or for several (a budget of accounts), valid for some days, and gives the account
 * a preset (what they can do, which libraries they see, how many devices they may have). Their
 * username and password are their own choice
 */
export default function SignupLinkSection() {
	const { t } = useTranslate()
	const [made, setMade] = useState<MadeLink | null>(null)

	return (
		<Section
			id="signups"
			title={t(accessKey('signups.title'))}
			description={t(accessKey('signups.description'))}
		>
			<Suspense fallback={null}>
				<SignupForm onMade={setMade} />
			</Suspense>
			{made && <MadeLinkBox made={made} onDismiss={() => setMade(null)} />}
			<Suspense fallback={null}>
				<SignupList />
			</Suspense>
		</Section>
	)
}
