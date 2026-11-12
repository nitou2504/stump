import { Button, CheckBox, Dialog, Text } from '@stump/components'
import { useState } from 'react'
import { toast } from 'sonner'

import { useTranslate } from '@/hooks/useTranslate'

import { useAccessActions } from './graphql'
import { accessKey } from './keys'

const errorMessage = (error: unknown) =>
	error instanceof Error && error.message ? error.message : undefined

/** What the dialog is about: a signup link (that may have made accounts) or a device link */
export type Revoking =
	| { kind: 'signup'; id: string; label: string; accounts: number }
	| { kind: 'device'; id: string; label: string }

/**
 * Stops a link so it can't be opened any more. For a signup link that made accounts, the owner
 * may also turn those accounts off (their devices stop working and their sessions end); the
 * accounts themselves are never deleted here
 */
export default function RevokeLinkDialog({
	link,
	onClose,
}: {
	link: Revoking | null
	onClose: () => void
}) {
	const { t, translate } = useTranslate()
	const actions = useAccessActions()
	const [disableAccounts, setDisableAccounts] = useState(false)

	const close = () => {
		setDisableAccounts(false)
		onClose()
	}

	const confirm = async () => {
		if (!link) return
		try {
			if (link.kind === 'signup') {
				await actions.revokeSignupLink(link.id, disableAccounts)
			} else {
				await actions.revokeInvite(link.id)
			}
			toast.success(t(accessKey('revoke.done'), { label: link.label }))
			close()
		} catch (error) {
			toast.error(t(accessKey('revoke.error')), { description: errorMessage(error) })
		}
	}

	return (
		<Dialog open={!!link} onOpenChange={(open) => !open && close()}>
			<Dialog.Content size="sm" data-testid="access-revoke-link">
				<Dialog.Header>
					<Dialog.Title>{t(accessKey('revoke.title'), { label: link?.label ?? '' })}</Dialog.Title>
					<Dialog.Close onClick={close} />
				</Dialog.Header>
				<div className="gap-4 flex flex-col">
					<Text size="sm">{t(accessKey('revoke.body'))}</Text>
					{link?.kind === 'signup' && link.accounts > 0 && (
						<CheckBox
							id="access-revoke-accounts"
							label={t(accessKey('revoke.disableAccounts'), { count: link.accounts })}
							checked={disableAccounts}
							onCheckedChange={(checked) => setDisableAccounts(checked === true)}
						/>
					)}
				</div>
				<Dialog.Footer>
					<Button variant="outline" onClick={close}>
						{translate('shared.common.cancel')}
					</Button>
					<Button variant="destructive" onClick={confirm} isLoading={actions.isPending}>
						{t(accessKey('revoke.confirm'))}
					</Button>
				</Dialog.Footer>
			</Dialog.Content>
		</Dialog>
	)
}
