import { zodResolver } from '@hookform/resolvers/zod'
import { useSDK } from '@stump/client'
import { Alert, AlertDescription, Button, Form, Input, PasswordInput } from '@stump/components'
import { isAxiosError } from '@stump/sdk'
import { useQuery } from '@tanstack/react-query'
import { ShieldAlert } from 'lucide-react'
import { useCallback, useMemo, useState } from 'react'
import { FieldValues, useForm } from 'react-hook-form'
import { Link, Navigate } from 'react-router'
import { z } from 'zod'

import { useTranslate } from '@/hooks/useTranslate'

import AuthLogo from './AuthLogo'
import { forgetJoinToken, joinToken } from './joinToken'

/** The shortest a username and a password may be (the server checks these and the rest) */
const MIN_USERNAME = 3
const MIN_PASSWORD = 10

/**
 * The join screen of an invite link, on the public address: the person chooses a username and a
 * password, and the server makes the account (with what the link gives), signs them in and sets the
 * pass of this device. The link's token comes from the address's fragment (see `joinToken`).
 *
 * Only the public address has invite links: anywhere else this goes to the sign-in screen. After the
 * account is made the page is loaded again (the pass is then there), never navigated within the
 * page that was served to a visitor with no pass
 */
export default function JoinScene() {
	const { t } = useTranslate()
	const { sdk } = useSDK()

	const claim = useQuery({
		queryKey: [sdk.server.keys.claimedStatus, 'join'],
		queryFn: () => sdk.server.claimedStatus(),
	})

	// read once: the address was already cleaned before the app rendered (apps/web), and this
	// does it too if nothing did
	const token = useMemo(() => joinToken(), [])
	// what the server said about the link or the account, and why this screen is where it is
	const [linkIsDead, setLinkIsDead] = useState(false)
	const [accountIsReady, setAccountIsReady] = useState(false)
	const [problem, setProblem] = useState<string | null>(null)
	const [isJoining, setIsJoining] = useState(false)

	const schema = z
		.object({
			username: z
				.string()
				.trim()
				.min(1, { message: t('authScene.form.validation.missingUsername') })
				.min(MIN_USERNAME, { message: t('authScene.join.rules') }),
			password: z
				.string()
				.min(1, { message: t('authScene.form.validation.missingPassword') })
				.min(MIN_PASSWORD, { message: t('authScene.join.rules') }),
			password2: z.string(),
		})
		.refine((values) => values.password === values.password2, {
			message: t('authScene.join.mismatch'),
			path: ['password2'],
		})
	const form = useForm<z.infer<typeof schema>>({ resolver: zodResolver(schema) })
	const { errors } = form.formState

	const join = useCallback(
		async ({ username, password }: FieldValues) => {
			if (!token) {
				setLinkIsDead(true)
				return
			}
			setProblem(null)
			setIsJoining(true)
			try {
				await sdk.auth.join({ token, username: String(username).trim(), password })
				forgetJoinToken()
				// the pass of this device is set: load the app again, as a signed-in one
				window.location.assign('/')
			} catch (error) {
				setIsJoining(false)
				const status = isAxiosError(error) ? error.response?.status : undefined
				const message = isAxiosError(error)
					? (error.response?.data as { message?: unknown } | undefined)?.message
					: undefined
				if (status === 404) {
					setLinkIsDead(true)
				} else if (status === 409) {
					// the account exists, the session could not start: they sign in with it
					setAccountIsReady(true)
				} else if (status === 422 && typeof message === 'string' && message) {
					// the visitor holds a valid link: the rule that was broken is told as the server words it
					setProblem(message)
				} else if (status === 429) {
					setProblem(t('authScene.public.tooMany'))
				} else {
					setProblem(t('authScene.join.generic'))
				}
			}
		},
		[sdk, t, token],
	)

	if (claim.isLoading) {
		return null
	}
	// invite links are for the public address only
	if (claim.data?.data.isPublic !== true) {
		return <Navigate to="/auth" replace />
	}

	const noLink = !token || linkIsDead
	const message = accountIsReady
		? t('authScene.join.accountReady')
		: noLink
			? t('authScene.join.badLink')
			: problem

	return (
		<div className="flex h-full w-full items-center bg-background">
			<div className="px-6 sm:px-0 w-screen shrink-0">
				<div className="gap-8 p-4 flex h-full w-full flex-col items-center justify-center bg-background">
					<AuthLogo />

					{message && (
						<Alert
							variant="destructive"
							className="sm:max-w-md md:max-w-lg"
							role="alert"
							data-testid="joinProblem"
						>
							<ShieldAlert />
							<AlertDescription>{message}</AlertDescription>
						</Alert>
					)}

					{!noLink && !accountIsReady && (
						<Form form={form} onSubmit={join} className="sm:min-w-80 sm:w-[unset] w-full">
							<Input
								id="username"
								label={t('authScene.join.username')}
								autoComplete="username"
								autoCapitalize="off"
								autoFocus
								fullWidth
								errorMessage={errors.username?.message}
								{...form.register('username')}
							/>

							<PasswordInput
								id="password"
								label={t('authScene.join.password')}
								type="password"
								autoComplete="new-password"
								fullWidth
								errorMessage={errors.password?.message}
								{...form.register('password')}
							/>

							<PasswordInput
								id="password2"
								label={t('authScene.join.passwordAgain')}
								type="password"
								autoComplete="new-password"
								fullWidth
								errorMessage={errors.password2?.message}
								{...form.register('password2')}
							/>

							<p className="text-xs text-muted-foreground">{t('authScene.join.rules')}</p>

							<Button data-testid="joinButton" type="submit" isLoading={isJoining} className="mt-2">
								{t('authScene.join.button')}
							</Button>
						</Form>
					)}

					<p className="text-sm text-center text-muted-foreground">
						<Link to="/auth" className="underline-offset-4 hover:underline">
							{t('authScene.join.haveAccount')}
						</Link>
					</p>
				</div>
			</div>
		</div>
	)
}
