import {
	Alert,
	AlertDescription,
	Button,
	ButtonOrLink,
	CheckBox,
	Dialog,
	RadioGroup,
	Text,
} from '@stump/components'
import { useQuery } from '@tanstack/react-query'
import { useState } from 'react'
import { toast } from 'sonner'

import { useTranslate } from '@/hooks/useTranslate'
import { usePaths } from '@/paths'

import {
	type AccessPersonRow,
	PUBLIC_ACCESS_KEY,
	useAccessActions,
	useLibraryChoices,
	usePresetDefaults,
	useUserLoader,
} from './graphql'
import { accessKey } from './keys'
import { FRIEND_PERMISSIONS, permissionKey } from './permissions'

const errorMessage = (error: unknown) =>
	error instanceof Error && error.message ? error.message : undefined

type Props = {
	/** The person to let in; nothing closes the dialog */
	person: AccessPersonRow | null
	onClose: () => void
	/** After letting somebody in: the owner chose to set a new password for them */
	onSetPassword: (person: { id: string; username: string }) => void
}

/**
 * Letting somebody in from the internet. It says what they will be able to do and see. When the
 * account holds permissions that are not for the internet, those are named and the owner can take
 * them off right here (everything else of the account stays as it is) and let the person in in
 * one step; otherwise the owner chooses whether they see everything they do now or only the
 * default library, and may go straight on to setting a new password (recommended for an account
 * that was made before the internet was involved)
 */
export default function LetInDialog({ person, onClose, onSetPassword }: Props) {
	const { t, translate } = useTranslate()
	const paths = usePaths()
	const defaults = usePresetDefaults()
	const libraries = useLibraryChoices()
	const actions = useAccessActions()
	const load = useUserLoader()

	// what the owner picked, if they did: somebody who is not in yet starts on the default library
	// alone, somebody who is already in starts on *leave as they are* (the dialog is also the way to
	// fix an account, and must not cut the libraries of a person who has them)
	const [picked, setPicked] = useState<boolean | null>(null)
	const [newPassword, setNewPassword] = useState(true)

	const row = person
	const onlyDefault = picked ?? !person?.publicSignin
	const account = row?.person
	const extras = row?.beyondFriendSet ?? []
	const defaultLibrary = defaults.libraries[0]

	// what the account can do, from the account itself (the page's own lists don't carry it)
	const user = useQuery({
		queryKey: [PUBLIC_ACCESS_KEY, 'userToEdit', account?.id],
		queryFn: () => load(account?.id ?? ''),
		enabled: !!account,
	})
	const can = (user.data?.permissions ?? [])
		.filter((permission) => (FRIEND_PERMISSIONS as readonly string[]).includes(permission))
		.map((permission) => t(accessKey(`signups.permission.${permissionKey(permission)}`)))

	// what they see: the default library alone, or what they see now
	const sees =
		onlyDefault && defaultLibrary
			? defaultLibrary.name
			: !row || row.hiddenLibraries === 0
				? t(accessKey('letIn.everything'))
				: row.hiddenLibraries >= libraries.length
					? t(accessKey('letIn.nothing'))
					: t(accessKey('letIn.some'), { count: libraries.length - row.hiddenLibraries })

	const close = () => {
		setPicked(null)
		setNewPassword(true)
		onClose()
	}

	const confirm = async () => {
		if (!row || !account) return
		try {
			if (extras.length > 0) {
				// the account as it is, less what keeps it off the internet
				const loaded = user.data ?? (await load(account.id))
				await actions.removePermissions(loaded, extras)
			}
			await actions.setPersonAccess({
				personId: account.id,
				publicSignin: true,
				onlyLibraryIds: onlyDefault && defaultLibrary ? [defaultLibrary.id] : undefined,
			})
			toast.success(t(accessKey('letIn.done'), { name: account.username }))
			if (newPassword) onSetPassword({ id: account.id, username: account.username })
			close()
		} catch (error) {
			toast.error(t(accessKey('letIn.error')), { description: errorMessage(error) })
		}
	}

	return (
		<Dialog open={!!row} onOpenChange={(open) => !open && close()}>
			<Dialog.Content size="sm" data-testid="access-letin">
				<Dialog.Header>
					<Dialog.Title>
						{t(accessKey('letIn.title'), { name: account?.username ?? '' })}
					</Dialog.Title>
					<Dialog.Close onClick={close} />
				</Dialog.Header>

				<div className="gap-4 flex flex-col">
					<Text size="sm">{t(accessKey('letIn.body'), { limit: row?.maxDevices ?? 3 })}</Text>

					{extras.length > 0 ? (
						<Alert variant="warning" data-testid="access-letin-extras">
							<AlertDescription>
								{t(accessKey('letIn.extras'), {
									name: account?.username ?? '',
									extras: extras
										.map((permission) => t(`userPermissions.${permission}.label`))
										.join(', '),
								})}
							</AlertDescription>
						</Alert>
					) : (
						can.length > 0 && (
							<Text size="sm">{t(accessKey('letIn.can'), { can: can.join(', ') })}</Text>
						)
					)}

					<Text size="sm">{t(accessKey('letIn.sees'), { libraries: sees })}</Text>

					{defaultLibrary && (
						<RadioGroup
							aria-label={t(accessKey('letIn.libraries'))}
							value={onlyDefault ? 'only' : 'leave'}
							onValueChange={(value) => setPicked(value === 'only')}
							className="gap-2"
						>
							<RadioGroup.CardItem
								value="only"
								label={t(accessKey('letIn.onlyDefault'), { name: defaultLibrary.name })}
							/>
							<RadioGroup.CardItem value="leave" label={t(accessKey('letIn.keepLibraries'))} />
						</RadioGroup>
					)}

					<CheckBox
						id="access-letin-password"
						label={t(accessKey('letIn.setPassword'))}
						checked={newPassword}
						onCheckedChange={(checked) => setNewPassword(checked === true)}
					/>
				</div>

				<Dialog.Footer>
					<Button variant="outline" onClick={close}>
						{translate('shared.common.cancel')}
					</Button>
					{extras.length > 0 && account && (
						<ButtonOrLink variant="outline" href={paths.updateUser(account.id)}>
							{t(accessKey('person.editAccount'))}
						</ButtonOrLink>
					)}
					<Button onClick={confirm} isLoading={actions.isPending}>
						{t(accessKey(extras.length > 0 ? 'letIn.removeAndLetIn' : 'letIn.letIn'))}
					</Button>
				</Dialog.Footer>
			</Dialog.Content>
		</Dialog>
	)
}
