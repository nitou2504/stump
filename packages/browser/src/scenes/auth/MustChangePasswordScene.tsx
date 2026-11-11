import { useGraphQLMutation, useSDK } from '@stump/client'
import {
	Alert,
	AlertDescription,
	Button,
	Card,
	Heading,
	PasswordInput,
	Text,
} from '@stump/components'
import { graphql } from '@stump/graphql'
import { useQueryClient } from '@tanstack/react-query'
import { FormEvent, useState } from 'react'
import { useNavigate } from 'react-router'
import { toast } from 'sonner'

import { useTranslate } from '@/hooks/useTranslate'
import { useUserStore } from '@/stores'

/** The shortest password the server takes */
const MIN_LENGTH = 8

const mutation = graphql(`
	mutation ChooseOwnPassword($input: UpdateUserInput!) {
		updateViewer(input: $input) {
			id
			username
		}
	}
`)

const KEY = 'authScene.mustChangePassword'

/**
 * What a person sees, instead of Stump, when the server owner set their password: it only gets
 * them in, and until they choose their own the server answers nothing else (so the app isn't
 * shown, it could only fail). It asks for the new password twice, saves it, and goes on
 */
export default function MustChangePasswordScene() {
	const { t } = useTranslate()
	const { sdk } = useSDK()
	const client = useQueryClient()
	const navigate = useNavigate()
	const user = useUserStore((state) => state.user)
	const setUser = useUserStore((state) => state.setUser)

	const [password, setPassword] = useState('')
	const [repeat, setRepeat] = useState('')
	const [error, setError] = useState<string | null>(null)

	const { mutateAsync, isPending } = useGraphQLMutation(mutation)

	if (!user) return null

	const tooShort = password.length > 0 && password.length < MIN_LENGTH
	const mismatch = repeat.length > 0 && password !== repeat
	const canSubmit = password.length >= MIN_LENGTH && password === repeat && !isPending

	const submit = async (event: FormEvent) => {
		event.preventDefault()
		if (!canSubmit) return
		setError(null)
		try {
			// the same input the profile form sends: the name and the rest stay as they are
			await mutateAsync({
				input: {
					username: user.username,
					password,
					permissions: user.permissions,
					ageRestriction: user.ageRestriction || null,
					maxSessionsAllowed: user.maxSessionsAllowed,
				},
			})
			toast.success(t(`${KEY}.success`))
			setUser({ ...user, mustChangePassword: false })
			// everything asked while the password was still the given one was refused
			client.clear()
		} catch (failure) {
			// the server says why ("choose a different one") in its own words
			setError(failure instanceof Error && failure.message ? failure.message : t(`${KEY}.error`))
		}
	}

	const signOut = async () => {
		try {
			await sdk.auth.logout()
		} finally {
			client.clear()
			setUser(null)
			navigate('/auth')
		}
	}

	return (
		<div className="p-4 flex min-h-screen w-full items-center justify-center bg-background">
			<Card className="gap-6 p-6 max-w-md flex w-full flex-col">
				<div className="gap-2 flex flex-col">
					<Heading size="md">{t(`${KEY}.title`)}</Heading>
					<Text size="sm" variant="muted">
						{t(`${KEY}.description`)}
					</Text>
				</div>

				<form onSubmit={submit} className="gap-4 flex flex-col">
					<PasswordInput
						id="new-password"
						label={t(`${KEY}.newPassword`)}
						autoComplete="new-password"
						value={password}
						onChange={(event) => setPassword(event.target.value)}
						errorMessage={tooShort ? t(`${KEY}.tooShort`, { min: MIN_LENGTH }) : undefined}
						fullWidth
						autoFocus
					/>
					<PasswordInput
						id="repeat-password"
						label={t(`${KEY}.confirmPassword`)}
						autoComplete="new-password"
						value={repeat}
						onChange={(event) => setRepeat(event.target.value)}
						errorMessage={mismatch ? t(`${KEY}.mismatch`) : undefined}
						fullWidth
					/>

					{error && (
						<Alert variant="destructive">
							<AlertDescription>{error}</AlertDescription>
						</Alert>
					)}

					<div className="gap-2 sm:flex-row flex flex-col-reverse justify-between">
						<Button type="button" variant="ghost" onClick={signOut}>
							{t(`${KEY}.signOut`)}
						</Button>
						<Button type="submit" disabled={!canSubmit} isLoading={isPending}>
							{t(`${KEY}.submit`)}
						</Button>
					</div>
				</form>
			</Card>
		</div>
	)
}
