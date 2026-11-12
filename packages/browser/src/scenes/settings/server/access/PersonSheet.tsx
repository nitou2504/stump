import {
	Alert,
	AlertDescription,
	Badge,
	Button,
	ButtonOrLink,
	CheckBox,
	NativeSelect,
	NewCard,
	RawSwitch,
	Text,
} from '@stump/components'
import { AccessDeviceKind } from '@stump/graphql'
import { BookOpen, ChevronDown, Monitor, Terminal } from 'lucide-react'
import { Suspense, useState } from 'react'
import { toast } from 'sonner'

import { useTranslate } from '@/hooks/useTranslate'
import { usePaths } from '@/paths'

import DeviceLinkForm from './DeviceLinkForm'
import { agentLabel, formatAgo, formatWhen } from './format'
import {
	type AccessDevice,
	type AccessPersonRow,
	useAccessActions,
	useAccessBlocked,
	useAccessDevices,
	useAccessPeople,
	useLibraryChoices,
} from './graphql'
import { accessKey } from './keys'
import { MAX_DEVICES, MIN_DEVICES } from './permissions'
import SidePanel from './SidePanel'

const errorMessage = (error: unknown) =>
	error instanceof Error && error.message ? error.message : undefined

type Props = {
	/** Whose sheet is open; nothing closes it */
	personId: string | null
	onClose: () => void
	/** Turning the switch on: the let-in dialog says what they get and asks first */
	onLetIn: (row: AccessPersonRow) => void
	/** The dialog that sets their password */
	onSetPassword: (person: { id: string; username: string }) => void
	/** A dialog is on top: the sheet steps aside (it comes back when the dialog closes) */
	hidden?: boolean
}

/** What kind of device it is, in words and as an icon */
const KIND_ICON = {
	terminal: Terminal,
	readingApp: BookOpen,
	browser: Monitor,
} as const

const kindOf = (device: AccessDevice): keyof typeof KIND_ICON =>
	device.legacy ? 'terminal' : device.kind === AccessDeviceKind.Opds ? 'readingApp' : 'browser'

type Ask =
	| { kind: 'off' }
	| { kind: 'removeDevice'; device: AccessDevice }
	| { kind: 'signOutAll' }
	| { kind: 'removeAll' }
	| null

/** One device: its name and kind, when it was used, and a click for the rest */
function DeviceRow({
	device,
	onAllowMoreTries,
	onRemove,
	onRestore,
}: {
	device: AccessDevice
	onAllowMoreTries?: () => void
	onRemove?: () => void
	onRestore?: () => void
}) {
	const { t } = useTranslate()
	const [open, setOpen] = useState(false)
	const kind = kindOf(device)
	const Icon = KIND_ICON[kind]

	return (
		<div data-testid={`access-device-${device.id}`} className="px-4 py-3">
			<div className="gap-3 flex items-center">
				<button
					type="button"
					aria-expanded={open}
					onClick={() => setOpen((next) => !next)}
					className="gap-3 min-w-0 flex flex-1 items-center text-left"
				>
					<Icon aria-hidden className="h-4 w-4 shrink-0 text-muted-foreground" />
					<span className="min-w-0 flex-1">
						<span className="gap-2 flex flex-wrap items-center">
							<Text size="sm" className="font-medium">
								{device.label}
							</Text>
							<Badge size="xs" variant="secondary">
								{t(accessKey(`person.kind.${kind}`))}
							</Badge>
						</span>
						<Text size="xs" variant="muted" title={formatWhen(device.lastSeenAt)}>
							{onRestore
								? `${t(accessKey('ended.deviceRemoved'))} · ${formatWhen(device.revokedAt)}`
								: t(accessKey('person.used'), {
										when: formatAgo(device.lastSeenAt ?? device.createdAt),
									})}
						</Text>
					</span>
					<ChevronDown
						aria-hidden
						className={open ? 'h-4 w-4 shrink-0 rotate-180' : 'h-4 w-4 shrink-0'}
					/>
				</button>
				{onRestore && (
					<Button variant="outline" size="sm" onClick={onRestore}>
						{t(accessKey('person.restore'))}
					</Button>
				)}
			</div>
			{open && (
				<div className="gap-3 mt-2 pl-7 flex flex-col items-start">
					<Text size="xs" variant="muted" data-testid="access-device-detail">
						{t(accessKey('person.firstSeen'), {
							when: formatWhen(device.firstSeenAt ?? device.createdAt),
						})}
						{` · ${device.lastIp || '–'} · ${agentLabel(device.lastAgent)}`}
					</Text>
					{(onAllowMoreTries || onRemove) && (
						<div className="gap-2 flex flex-wrap">
							{onAllowMoreTries && (
								<Button variant="outline" size="sm" onClick={onAllowMoreTries}>
									{t(accessKey('person.allowMoreTries'))}
								</Button>
							)}
							{onRemove && (
								<Button variant="ghost" size="sm" className="text-destructive" onClick={onRemove}>
									{t(accessKey('person.removeDevice'))}
								</Button>
							)}
						</div>
					)}
					{onAllowMoreTries && (
						<Text size="xs" variant="muted">
							{t(accessKey('person.allowMoreTriesHelp'))}
						</Text>
					)}
				</div>
			)}
		</div>
	)
}

/** The sheet's body: everything about one person */
function Body({
	row,
	onLetIn,
	onSetPassword,
	onClose,
}: {
	row: AccessPersonRow
	onLetIn: Props['onLetIn']
	onSetPassword: Props['onSetPassword']
	onClose: () => void
}) {
	const { t, translate } = useTranslate()
	const paths = usePaths()
	const actions = useAccessActions()
	const libraries = useLibraryChoices()
	const blocked = useAccessBlocked().find(({ person }) => person.id === row.person.id)
	const person = row.person

	const active = useAccessDevices(person.id, false).nodes
	const everyDevice = useAccessDevices(person.id, true).nodes
	const removed = everyDevice.filter((device) => device.revokedAt)

	const [ask, setAsk] = useState<Ask>(null)
	const [addLink, setAddLink] = useState(false)
	const [showRemoved, setShowRemoved] = useState(false)
	const [changeLibraries, setChangeLibraries] = useState(false)

	// the libraries they see: every one that doesn't list them as kept out
	const seen = libraries.filter(
		({ excludedUsers }) => !excludedUsers.some(({ id }) => id === person.id),
	)
	const [picked, setPicked] = useState<string[] | null>(null)
	const chosen = picked ?? seen.map(({ id }) => id)
	const sees =
		seen.length === libraries.length
			? t(accessKey('person.seesEverything'))
			: seen.length === 0
				? t(accessKey('person.seesNothing'))
				: t(accessKey('person.sees'), { libraries: seen.map(({ name }) => name).join(', ') })

	const guard = async (work: () => Promise<unknown>, done?: string) => {
		try {
			await work()
			if (done) toast.success(done)
			setAsk(null)
		} catch (error) {
			toast.error(t(accessKey('person.error')), { description: errorMessage(error) })
		}
	}

	// what is being asked, in words: shown in the sheet itself (a dialog on top of a sheet would
	// have two focus traps fighting for the keyboard)
	const confirmation = !ask
		? null
		: ask.kind === 'off'
			? {
					text: t(accessKey('letIn.turnOff'), { name: person.username }),
					label: t(accessKey('person.turnOff')),
					destructive: false,
					run: () => actions.setPersonAccess({ personId: person.id, publicSignin: false }),
				}
			: ask.kind === 'removeDevice'
				? {
						text: t(accessKey('person.removeDeviceConfirm'), { label: ask.device.label }),
						label: t(accessKey('person.removeDevice')),
						destructive: true,
						run: () => actions.revokeDevice(ask.device.id),
					}
				: ask.kind === 'signOutAll'
					? {
							text: t(accessKey('person.signOutAllConfirm'), { name: person.username }),
							label: t(accessKey('person.signOutAll')),
							destructive: false,
							run: () => actions.signOutEverywhere(person.id),
						}
					: {
							text: t(accessKey('person.removeAllConfirm'), { name: person.username }),
							label: t(accessKey('person.removeAll')),
							destructive: true,
							run: () => actions.revokePerson(person.id),
						}

	return (
		<div className="gap-6 flex flex-col" data-testid="access-person-sheet">
			{confirmation && (
				<div
					role="alert"
					data-testid="access-confirm"
					className="gap-3 p-4 flex flex-col rounded-xl border border-border bg-muted/40"
				>
					<Text size="sm">{confirmation.text}</Text>
					<div className="gap-2 flex">
						<Button variant="outline" size="sm" onClick={() => setAsk(null)}>
							{translate('shared.common.cancel')}
						</Button>
						<Button
							size="sm"
							variant={confirmation.destructive ? 'destructive' : 'default'}
							isLoading={actions.isPending}
							onClick={() => guard(confirmation.run)}
						>
							{confirmation.label}
						</Button>
					</div>
				</div>
			)}

			<NewCard>
				<NewCard.Row
					label={t(accessKey('person.internet'))}
					description={
						row.publicSignin
							? t(accessKey('person.internetHelp'), { limit: row.maxDevices })
							: undefined
					}
					className="flex-row items-center justify-between"
				>
					<RawSwitch
						aria-label={t(accessKey('person.internet'))}
						checked={row.publicSignin}
						disabled={actions.isPending}
						onCheckedChange={(checked) => (checked ? onLetIn(row) : setAsk({ kind: 'off' }))}
					/>
				</NewCard.Row>
			</NewCard>

			<NewCard
				label={t(accessKey('person.devices'), { active: active.length, limit: row.maxDevices })}
			>
				{active.length === 0 ? (
					<Text size="sm" variant="muted" className="px-4 py-4">
						{t(accessKey('person.noDevices'))}
					</Text>
				) : (
					<div className="divide-y divide-border/70">
						{active.map((device) => (
							<DeviceRow
								key={device.id}
								device={device}
								onAllowMoreTries={() =>
									guard(() => actions.resetBudget(device.id), t(accessKey('person.triesAllowed')))
								}
								onRemove={() => setAsk({ kind: 'removeDevice', device })}
							/>
						))}
					</div>
				)}
				<NewCard.Row label={t(accessKey('person.limit'))} htmlFor="access-person-limit">
					<NativeSelect
						id="access-person-limit"
						value={row.maxDevices}
						disabled={actions.isPending}
						onChange={(event) =>
							guard(() =>
								actions.setPersonAccess({
									personId: person.id,
									maxDevices: Number(event.target.value),
								}),
							)
						}
						options={Array.from({ length: MAX_DEVICES - MIN_DEVICES + 1 }, (_, index) => ({
							label: t(accessKey('person.devicesCount'), { count: MIN_DEVICES + index }),
							value: MIN_DEVICES + index,
						}))}
					/>
				</NewCard.Row>
				<div className="gap-2 px-4 py-3 flex flex-col items-start border-t border-border/70">
					{addLink ? (
						<DeviceLinkForm personId={person.id} onClose={() => setAddLink(false)} />
					) : (
						<Button variant="ghost" size="sm" onClick={() => setAddLink(true)}>
							{t(accessKey('person.addDeviceLink'))}
						</Button>
					)}
					{removed.length > 0 && (
						<Button
							variant="ghost"
							size="sm"
							aria-expanded={showRemoved}
							onClick={() => setShowRemoved((open) => !open)}
						>
							{t(accessKey('person.showRemoved'), { count: removed.length })}
						</Button>
					)}
				</div>
				{showRemoved && (
					<div className="divide-y divide-border/70 border-t border-border/70">
						{removed.map((device) => (
							<DeviceRow
								key={device.id}
								device={device}
								onRestore={() => guard(() => actions.restoreDevice(device.id))}
							/>
						))}
					</div>
				)}
			</NewCard>

			<NewCard label={t(accessKey('person.passwordTitle'))}>
				{person.locked && (
					<div className="px-4 pt-3">
						<Alert variant="warning" data-testid="access-person-locked">
							<AlertDescription className="gap-3 flex items-center justify-between">
								{t(accessKey('person.locked'), { count: blocked?.failuresLast24H ?? 0 })}
								<Button
									size="sm"
									variant="outline"
									onClick={() => guard(() => actions.unblock(person.id))}
								>
									{t(accessKey('person.unblock'))}
								</Button>
							</AlertDescription>
						</Alert>
					</div>
				)}
				<NewCard.Row
					label={t(accessKey('person.setPassword'))}
					onClick={() => onSetPassword({ id: person.id, username: person.username })}
					className="cursor-pointer"
				/>
				<NewCard.Row
					label={t(accessKey('person.signOutAll'))}
					onClick={() => setAsk({ kind: 'signOutAll' })}
					className="cursor-pointer"
				/>
			</NewCard>

			<NewCard label={t(accessKey('person.librariesTitle'))}>
				<NewCard.Row label={sees} className="flex-row items-center justify-between">
					<Button
						variant="outline"
						size="sm"
						onClick={() => setChangeLibraries((open) => !open)}
						aria-expanded={changeLibraries}
					>
						{t(accessKey('person.change'))}
					</Button>
				</NewCard.Row>
				{changeLibraries && (
					<div className="gap-3 px-4 py-3 flex flex-col border-t border-border/70">
						{libraries.map(({ id, name }) => (
							<CheckBox
								key={id}
								id={`access-person-library-${id}`}
								label={name}
								checked={chosen.includes(id)}
								onCheckedChange={() =>
									setPicked(
										chosen.includes(id) ? chosen.filter((other) => other !== id) : [...chosen, id],
									)
								}
							/>
						))}
						<div>
							<Button
								size="sm"
								isLoading={actions.isPending}
								onClick={() =>
									guard(async () => {
										await actions.setPersonAccess({ personId: person.id, onlyLibraryIds: chosen })
										setChangeLibraries(false)
										setPicked(null)
									})
								}
							>
								{translate('shared.common.save')}
							</Button>
						</div>
					</div>
				)}
				<NewCard.Row
					label={t(accessKey('person.hideNew'))}
					className="flex-row items-center justify-between"
				>
					<RawSwitch
						aria-label={t(accessKey('person.hideNew'))}
						checked={row.newLibrariesHidden}
						disabled={actions.isPending}
						onCheckedChange={(checked) =>
							guard(() =>
								actions.setPersonAccess({ personId: person.id, newLibrariesHidden: checked }),
							)
						}
					/>
				</NewCard.Row>
			</NewCard>

			<NewCard>
				<NewCard.Row label={t(accessKey('person.uploadLimits'))}>
					<ButtonOrLink
						variant="outline"
						size="sm"
						href={`${paths.bookApprovals()}?person=${person.id}#limits`}
						onClick={onClose}
					>
						{t(accessKey('person.open'))}
					</ButtonOrLink>
				</NewCard.Row>
				<NewCard.Row label={t(accessKey('person.editAccount'))}>
					<ButtonOrLink variant="outline" size="sm" href={paths.updateUser(person.id)}>
						{t(accessKey('person.open'))}
					</ButtonOrLink>
				</NewCard.Row>
			</NewCard>

			<div>
				<Button
					variant="ghost"
					size="sm"
					className="text-destructive"
					disabled={active.length === 0}
					onClick={() => setAsk({ kind: 'removeAll' })}
				>
					{t(accessKey('person.removeAll'))}
				</Button>
			</div>
		</div>
	)
}

/** When they were last in and from which of their devices, under their name */
function LastIn({ row }: { row: AccessPersonRow }) {
	const { t } = useTranslate()
	const devices = useAccessDevices(row.person.id, false).nodes
	const latest = devices
		.filter((device) => device.lastSeenAt)
		.sort(
			(a, b) => new Date(b.lastSeenAt ?? 0).getTime() - new Date(a.lastSeenAt ?? 0).getTime(),
		)[0]

	if (!row.lastSignInAt) return <>{t(accessKey('person.neverIn'))}</>
	return (
		<>
			{latest
				? t(accessKey('person.lastInFrom'), {
						when: formatAgo(row.lastSignInAt),
						device: latest.label,
					})
				: t(accessKey('person.lastIn'), { when: formatAgo(row.lastSignInAt) })}
		</>
	)
}

/**
 * Everything about one person in one place: whether they can sign in from the internet, their
 * devices (each can be allowed more tries, removed, or brought back; a device link adds another),
 * their password and sessions, the libraries they see, and the ways on to their upload limits and
 * their account. It opens from the people list, and from the address (`?person=`) so a warning
 * can point straight at it
 */
export default function PersonSheet({ personId, onClose, onLetIn, onSetPassword, hidden }: Props) {
	const people = useAccessPeople()
	const row = people.find(({ person }) => person.id === personId) ?? null
	const open = !!personId && !!row && !hidden

	return (
		<SidePanel
			open={open}
			onClose={onClose}
			testId="access-sheet"
			title={row?.person.username ?? ''}
			description={
				row && (
					<Suspense fallback={null}>
						<LastIn row={row} />
					</Suspense>
				)
			}
		>
			{row && (
				<Suspense fallback={null}>
					<Body row={row} onLetIn={onLetIn} onSetPassword={onSetPassword} onClose={onClose} />
				</Suspense>
			)}
		</SidePanel>
	)
}
