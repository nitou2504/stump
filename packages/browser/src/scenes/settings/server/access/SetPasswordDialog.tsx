import { Alert, AlertDescription, Button, Dialog, Input, Switch, Text } from '@stump/components'
import { Check, Copy, RefreshCw } from 'lucide-react'
import { FormEvent, useEffect, useState } from 'react'
import { toast } from 'sonner'

import { useTranslate } from '@/hooks/useTranslate'

import { copyText } from './copy'
import { useAccessActions } from './graphql'
import { accessKey } from './keys'
import { generatePassword, MAX_PASSWORD_LENGTH, MIN_PASSWORD_LENGTH } from './password'

type Props = {
	person: { id: string; username: string }
	isOpen: boolean
	onClose: () => void
}

const errorMessage = (error: unknown) =>
	error instanceof Error && error.message ? error.message : undefined

/**
 * Sets somebody's password. One is made for the owner (20 characters, none that can be mistaken
 * for another), who can write their own instead. By default the person must choose their own at
 * their next sign-in, so the one set here is only a way in. It ends their sessions, and it is
 * shown once, after it was set: the field is emptied when the dialog closes
 */
export default function SetPasswordDialog({ person, isOpen, onClose }: Props) {
	const { t, translate } = useTranslate()
	const actions = useAccessActions()

	const [password, setPassword] = useState('')
	const [requireChange, setRequireChange] = useState(true)
	const [done, setDone] = useState(false)
	const [copied, setCopied] = useState(false)
	const [error, setError] = useState<string | null>(null)

	// a new password each time it opens; nothing of it is kept once it closes
	useEffect(() => {
		if (isOpen) {
			setPassword(generatePassword())
			setRequireChange(true)
			setDone(false)
			setCopied(false)
			setError(null)
		} else {
			setPassword('')
		}
	}, [isOpen])

	const length = password.length
	const isValid = length >= MIN_PASSWORD_LENGTH && length <= MAX_PASSWORD_LENGTH

	const submit = async (event: FormEvent) => {
		event.preventDefault()
		if (!isValid) return
		setError(null)
		try {
			await actions.setPassword(person.id, password, requireChange)
			setDone(true)
		} catch (failure) {
			// the server's words say what is wrong ("may not change their password, ...")
			setError(errorMessage(failure) ?? t(accessKey('people.password.error')))
		}
	}

	const copy = async () => {
		const worked = await copyText(password)
		setCopied(worked)
		if (!worked) toast.error(t(accessKey('people.password.copyFailed')))
	}

	return (
		<Dialog open={isOpen} onOpenChange={(nowOpen) => !nowOpen && onClose()}>
			<Dialog.Content size="sm">
				<Dialog.Header>
					<Dialog.Title>
						{t(accessKey('people.password.title'), { name: person.username })}
					</Dialog.Title>
					<Dialog.Close onClick={onClose} />
				</Dialog.Header>

				{done ? (
					<div data-testid="access-password-done" className="gap-3 flex flex-col">
						<Text size="sm">{t(accessKey('people.password.done'), { name: person.username })}</Text>
						<div className="gap-2 flex">
							<Input
								aria-label={t(accessKey('people.password.label'))}
								value={password}
								readOnly
								fullWidth
								onFocus={(event) => event.currentTarget.select()}
							/>
							<Button onClick={copy} className="shrink-0">
								{copied ? <Check className="h-4 w-4" /> : <Copy className="h-4 w-4" />}
								{t(accessKey('people.password.copy'))}
							</Button>
						</div>
						<Text size="sm" variant="muted">
							{t(
								accessKey(
									requireChange ? 'people.password.doneMustChange' : 'people.password.doneKept',
								),
							)}
						</Text>
					</div>
				) : (
					<form id="access-password-form" onSubmit={submit} className="gap-4 flex flex-col">
						<div className="gap-2 flex flex-col">
							<Input
								id="access-password"
								label={t(accessKey('people.password.label'))}
								value={password}
								onChange={(event) => setPassword(event.target.value)}
								autoComplete="off"
								autoCapitalize="off"
								spellCheck={false}
								fullWidth
							/>
							<div>
								<Button
									type="button"
									size="sm"
									variant="ghost"
									onClick={() => setPassword(generatePassword())}
								>
									<RefreshCw className="h-4 w-4" />
									{t(accessKey('people.password.generate'))}
								</Button>
							</div>
							{!isValid && (
								<Text size="sm" variant="danger">
									{t(accessKey('people.password.length'), {
										min: MIN_PASSWORD_LENGTH,
										max: MAX_PASSWORD_LENGTH,
									})}
								</Text>
							)}
						</div>

						<div className="gap-1 flex flex-col">
							<Switch
								id="access-require-change"
								label={t(accessKey('people.password.requireChange'))}
								checked={requireChange}
								onCheckedChange={setRequireChange}
							/>
							<Text size="sm" variant="muted">
								{t(accessKey('people.password.requireChangeHint'))}
							</Text>
						</div>

						<Text size="sm" variant="muted">
							{t(accessKey('people.password.signsOut'))}
						</Text>

						{error && (
							<Alert variant="destructive">
								<AlertDescription>{error}</AlertDescription>
							</Alert>
						)}
					</form>
				)}

				<Dialog.Footer>
					{done ? (
						<Button onClick={onClose}>{t(accessKey('people.password.close'))}</Button>
					) : (
						<>
							<Button variant="outline" onClick={onClose}>
								{translate('shared.common.cancel')}
							</Button>
							<Button
								type="submit"
								form="access-password-form"
								disabled={!isValid || actions.isPending}
								isLoading={actions.isPending}
							>
								{t(accessKey('people.password.set'))}
							</Button>
						</>
					)}
				</Dialog.Footer>
			</Dialog.Content>
		</Dialog>
	)
}
