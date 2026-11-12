import {
	Alert,
	AlertDescription,
	Badge,
	Button,
	ButtonOrLink,
	CheckBox,
	Dialog,
	NativeSelect,
	Switch,
	Text,
} from '@stump/components'
import { Minus, Plus } from 'lucide-react'
import { Suspense, useState } from 'react'
import { toast } from 'sonner'

import { useTranslate } from '@/hooks/useTranslate'
import { usePaths } from '@/paths'

import { formatWhen } from './format'
import {
	type AccessPersonRow,
	useAccessActions,
	useAccessPeople,
	usePresetDefaults,
} from './graphql'
import { accessKey } from './keys'
import Section, { Empty, Row, RowList } from './Section'
import SetPasswordDialog from './SetPasswordDialog'

const errorMessage = (error: unknown) =>
	error instanceof Error && error.message ? error.message : undefined

/** The fewest and most devices a person may have (the server holds them to it too) */
export const MIN_DEVICES = 1
export const MAX_DEVICES = 10

/** `SCAN_LIBRARY` as the owner reads it: "Scan library" */
export const permissionWords = (permission: string) =>
	permission.toLowerCase().replace(/_/g, ' ').replace(/^./, (letter) => letter.toUpperCase())

type Choice = { person: AccessPersonRow; mode: 'on' | 'off' }

/**
 * Turning public sign-in on for somebody, or off. On: an account that holds permissions a friend
 * may not have is refused with the names of the permissions to remove first; otherwise the owner
 * says whether to leave what the person sees as it is or to narrow it to the default library, and
 * may go straight on to setting a new password (recommended for an account that existed before).
 * Off: the person can no longer use Stump from the internet; their devices stay unless the owner
 * says to remove them
 */
function PublicSigninDialog({
	choice,
	onClose,
	onSetPassword,
}: {
	choice: Choice | null
	onClose: () => void
	onSetPassword: (person: AccessPersonRow['person']) => void
}) {
	const { t, translate } = useTranslate()
	const paths = usePaths()
	const defaults = usePresetDefaults()
	const actions = useAccessActions()
	const [narrow, setNarrow] = useState(true)
	const [removeDevices, setRemoveDevices] = useState(false)
	const [newPassword, setNewPassword] = useState(true)

	const person = choice?.person.person
	const beyond = choice?.person.beyondFriendSet ?? []
	const defaultLibrary = defaults.libraries[0]

	const close = () => {
		setNarrow(true)
		setRemoveDevices(false)
		setNewPassword(true)
		onClose()
	}

	const confirm = async () => {
		if (!choice || !person) return
		try {
			if (choice.mode === 'on') {
				await actions.setPersonAccess({
					personId: person.id,
					publicSignin: true,
					onlyLibraryIds: narrow && defaultLibrary ? [defaultLibrary.id] : undefined,
				})
				toast.success(t(accessKey('people.publicSignin.turnedOn'), { name: person.username }))
				if (newPassword) onSetPassword(person)
			} else {
				await actions.setPersonAccess({ personId: person.id, publicSignin: false })
				if (removeDevices) await actions.revokePerson(person.id)
				toast.success(t(accessKey('people.publicSignin.turnedOff'), { name: person.username }))
			}
			close()
		} catch (error) {
			toast.error(t(accessKey('people.publicSignin.error')), { description: errorMessage(error) })
		}
	}

	return (
		<Dialog open={!!choice} onOpenChange={(open) => !open && close()}>
			<Dialog.Content size="sm">
				<Dialog.Header>
					<Dialog.Title>
						{t(
							accessKey(
								choice?.mode === 'off'
									? 'people.publicSignin.offTitle'
									: 'people.publicSignin.onTitle',
							),
							{ name: person?.username ?? '' },
						)}
					</Dialog.Title>
					<Dialog.Close onClick={close} />
				</Dialog.Header>

				{choice?.mode === 'on' && beyond.length > 0 && (
					<Alert variant="warning" data-testid="access-beyond">
						<AlertDescription>
							{t(accessKey('people.publicSignin.beyond'), {
								name: person?.username ?? '',
								permissions: beyond.map(permissionWords).join(', '),
							})}
						</AlertDescription>
					</Alert>
				)}

				{choice?.mode === 'on' && beyond.length === 0 && (
					<div className="gap-4 flex flex-col">
						<Text size="sm">
							{t(accessKey('people.publicSignin.onDescription'), { name: person?.username ?? '' })}
						</Text>
						{defaultLibrary && (
							<NativeSelect
								aria-label={t(accessKey('people.publicSignin.libraries'))}
								value={narrow ? 'only' : 'leave'}
								onChange={(event) => setNarrow(event.target.value === 'only')}
								options={[
									{
										label: t(accessKey('people.publicSignin.onlyLibrary'), {
											name: defaultLibrary.name,
										}),
										value: 'only',
									},
									{ label: t(accessKey('people.publicSignin.leaveLibraries')), value: 'leave' },
								]}
							/>
						)}
						<CheckBox
							id="access-publicsignin-password"
							label={t(accessKey('people.publicSignin.newPassword'))}
							description={t(accessKey('people.publicSignin.newPasswordHint'))}
							checked={newPassword}
							onCheckedChange={(checked) => setNewPassword(checked === true)}
						/>
					</div>
				)}

				{choice?.mode === 'off' && (
					<div className="gap-4 flex flex-col">
						<Text size="sm">
							{t(accessKey('people.publicSignin.offDescription'), { name: person?.username ?? '' })}
						</Text>
						{choice.person.activeDevices > 0 && (
							<Switch
								id="access-publicsignin-devices"
								label={t(accessKey('people.publicSignin.removeDevices'), {
									count: choice.person.activeDevices,
								})}
								checked={removeDevices}
								onCheckedChange={setRemoveDevices}
							/>
						)}
					</div>
				)}

				<Dialog.Footer>
					<Button variant="outline" onClick={close}>
						{translate('shared.common.cancel')}
					</Button>
					{choice?.mode === 'on' && beyond.length > 0 ? (
						<ButtonOrLink href={person ? paths.updateUser(person.id) : '#'}>
							{t(accessKey('people.manage'))}
						</ButtonOrLink>
					) : (
						<Button onClick={confirm} isLoading={actions.isPending}>
							{t(
								accessKey(
									choice?.mode === 'off'
										? 'people.publicSignin.turnOff'
										: 'people.publicSignin.turnOn',
								),
							)}
						</Button>
					)}
				</Dialog.Footer>
			</Dialog.Content>
		</Dialog>
	)
}

function PeopleList() {
	const { t } = useTranslate()
	const paths = usePaths()
	const people = useAccessPeople()
	const actions = useAccessActions()
	const [settingFor, setSettingFor] = useState<AccessPersonRow['person'] | null>(null)
	const [choice, setChoice] = useState<Choice | null>(null)

	const unblock = async (id: string, name: string) => {
		try {
			await actions.unblock(id)
			toast.success(t(accessKey('signIns.unblocked'), { name }))
		} catch (error) {
			toast.error(t(accessKey('signIns.errors.unblock')), { description: errorMessage(error) })
		}
	}

	const changeDevices = async (row: AccessPersonRow, by: number) => {
		const next = Math.min(MAX_DEVICES, Math.max(MIN_DEVICES, row.maxDevices + by))
		if (next === row.maxDevices) return
		try {
			await actions.setPersonAccess({ personId: row.person.id, maxDevices: next })
		} catch (error) {
			toast.error(t(accessKey('people.devices.error')), { description: errorMessage(error) })
		}
	}

	return (
		<>
			<RowList data-testid="access-people-list">
				{people.length === 0 && <Empty>{t(accessKey('people.empty'))}</Empty>}
				{people.map((row) => {
					const { person, lastSignInAt, mustChangePassword } = row
					const isFriend = !person.isServerOwner && !person.removed
					return (
						<Row key={person.id} data-testid={`access-people-${person.id}`}>
							<div className="min-w-0">
								<div className="gap-2 flex flex-wrap items-center">
									<Text size="sm" className="font-medium">
										{person.username}
									</Text>
									{person.isServerOwner && (
										<Badge size="xs" variant="primary">
											{t(accessKey('people.owner'))}
										</Badge>
									)}
									{person.locked && (
										<Badge size="xs" variant="warning">
											{t(accessKey('people.locked'))}
										</Badge>
									)}
									{mustChangePassword && (
										<Badge size="xs" variant="secondary">
											{t(accessKey('people.mustChange'))}
										</Badge>
									)}
									{isFriend && row.publicSignin && (
										<Badge size="xs" variant="success">
											{t(accessKey('people.publicSignin.badge'))}
										</Badge>
									)}
								</div>
								<Text size="xs" variant="muted" className="mt-1">
									{t(accessKey('people.lastSignIn'), {
										at: lastSignInAt ? formatWhen(lastSignInAt) : t(accessKey('people.never')),
									})}
								</Text>
								{isFriend && (
									<Text size="xs" variant="muted" className="mt-1" data-testid={`access-people-${person.id}-numbers`}>
										{t(accessKey('people.numbers'), {
											active: row.activeDevices,
											max: row.maxDevices,
											sessions: row.sessions,
											hidden: row.hiddenLibraries,
										})}
									</Text>
								)}
								{isFriend && row.beyondFriendSet.length > 0 && (
									<div
										data-testid={`access-people-${person.id}-beyond`}
										className="mt-2 gap-1.5 flex flex-wrap items-center"
									>
										<Text size="xs" variant="muted">
											{t(accessKey('people.beyond'))}
										</Text>
										{row.beyondFriendSet.map((permission) => (
											<Badge key={permission} size="xs" variant="warning">
												{permissionWords(permission)}
											</Badge>
										))}
									</div>
								)}
							</div>
							<div className="gap-3 flex flex-wrap items-center">
								{isFriend && (
									<>
										<Switch
											id={`access-publicsignin-${person.id}`}
											label={t(accessKey('people.publicSignin.label'))}
											checked={row.publicSignin}
											disabled={actions.isPending}
											onCheckedChange={(on) => setChoice({ person: row, mode: on ? 'on' : 'off' })}
										/>
										<div
											className="gap-1 flex items-center"
											role="group"
											aria-label={t(accessKey('people.devices.label'))}
										>
											<Button
												size="sm"
												variant="outline"
												aria-label={t(accessKey('people.devices.fewer'))}
												disabled={actions.isPending || row.maxDevices <= MIN_DEVICES}
												onClick={() => changeDevices(row, -1)}
											>
												<Minus className="h-3 w-3" />
											</Button>
											<Text size="sm" className="min-w-10 text-center" aria-live="polite">
												{row.maxDevices}
											</Text>
											<Button
												size="sm"
												variant="outline"
												aria-label={t(accessKey('people.devices.more'))}
												disabled={actions.isPending || row.maxDevices >= MAX_DEVICES}
												onClick={() => changeDevices(row, 1)}
											>
												<Plus className="h-3 w-3" />
											</Button>
										</div>
									</>
								)}
								{person.locked && (
									<Button
										size="sm"
										variant="outline"
										disabled={actions.isPending}
										onClick={() => unblock(person.id, person.username)}
									>
										{t(accessKey('signIns.unblock'))}
									</Button>
								)}
								{/* the owner's password is not set from here, and it is never used from the internet */}
								{isFriend && (
									<Button size="sm" variant="outline" onClick={() => setSettingFor(person)}>
										{t(accessKey('people.setPassword'))}
									</Button>
								)}
								{!person.removed && (
									<ButtonOrLink size="sm" variant="ghost" href={paths.updateUser(person.id)}>
										{t(accessKey('people.manage'))}
									</ButtonOrLink>
								)}
							</div>
						</Row>
					)
				})}
			</RowList>

			{/* only while it is open: it asks the server for the default library, which a closed one has no use for */}
			{choice && (
				<Suspense fallback={null}>
					<PublicSigninDialog
						choice={choice}
						onClose={() => setChoice(null)}
						onSetPassword={setSettingFor}
					/>
				</Suspense>
			)}

			{settingFor && (
				<SetPasswordDialog
					person={settingFor}
					isOpen={!!settingFor}
					onClose={() => setSettingFor(null)}
				/>
			)}
		</>
	)
}

/**
 * The accounts: who may come in from the internet (a switch each, refused with the names of what
 * to remove for an account that holds more than a friend may), how many devices each may have,
 * and the two things the owner needs for somebody who can't get in: a new password, or the way in
 * again
 */
export default function PeopleSection() {
	const { t } = useTranslate()

	return (
		<Section
			id="people"
			title={t(accessKey('people.title'))}
			description={t(accessKey('people.description'))}
		>
			<Suspense fallback={null}>
				<PeopleList />
			</Suspense>
		</Section>
	)
}
