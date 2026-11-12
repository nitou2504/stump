import { AuthUser, isAxiosError, isUser, PasswordUserInput } from '@stump/sdk'
import { useMutation, useQuery, useQueryClient, UseQueryOptions } from '@tanstack/react-query'
import { useEffect, useState } from 'react'

import { useClientContext } from '../context'
import { useSDK } from '../sdk'

type Params = Omit<UseQueryOptions<AuthUser>, 'queryKey' | 'queryFn'> & {
	additionalKeys?: string[]
}

export function useAuthQuery({ additionalKeys, ...options }: Params = {}) {
	const { sdk } = useSDK()
	const { data, error, isLoading, isFetching, isRefetching } = useQuery({
		queryKey: [sdk.auth.keys.me, ...(additionalKeys || [])],
		queryFn: async () => {
			const data = await sdk.auth.me()
			if (!data.id) {
				console.warn('Malformed response received from server', data)
				throw new Error('Malformed response received from server')
			}
			return data
		},
		throwOnError: false,
		...options,
	})

	return {
		error,
		isLoading: isLoading || isFetching || isRefetching,
		user: data,
	}
}

type UseLoginOrRegisterOptions = {
	/**
	 * Called with the signed-in user. `context.isPublic` is true on the public address (through the
	 * edge), where the screen goes on with a full page load instead of an in-app navigation
	 */
	onSuccess?: (data?: AuthUser | null | undefined, context?: { isPublic: boolean }) => void
	onError?: (data: unknown) => void
	refetchClaimed?: boolean
}

export function useLoginOrRegister({
	onSuccess,
	onError,
	refetchClaimed,
}: UseLoginOrRegisterOptions) {
	const [isClaimed, setIsClaimed] = useState(true)

	const client = useQueryClient()

	const { onAuthenticated } = useClientContext()
	const { sdk } = useSDK()
	const { data: claimCheck, isLoading: isCheckingClaimed } = useQuery({
		queryKey: [sdk.server.keys.claimedStatus, refetchClaimed],
		queryFn: () => sdk.server.claimedStatus(),
		retry: (failureCount, error) => {
			if (failureCount > 3) {
				return false
			} else {
				return isAxiosError(error) && error.code === 'ERR_NETWORK'
			}
		},
	})

	useEffect(() => {
		if (claimCheck?.data) {
			setIsClaimed(claimCheck.data.isClaimed)
		}
	}, [claimCheck])

	// the request came through the public edge: the sign-in screen shows its short version
	const isPublic = claimCheck?.data?.isPublic === true

	const {
		isPending: isLoggingIn,
		mutateAsync: loginUser,
		error: loginError,
	} = useMutation({
		mutationKey: [sdk.auth.keys.login],
		mutationFn: (params: PasswordUserInput) => sdk.auth.login(params),
		onError: (err) => {
			onError?.(err)
		},
		onSuccess: async (response) => {
			if ('forUser' in response && !!onAuthenticated) {
				const { forUser, ...token } = response
				await onAuthenticated(forUser, token)
				onSuccess?.(forUser, { isPublic })
			} else if (isUser(response)) {
				onSuccess?.(response, { isPublic })
			}
		},
	})

	const { isPending: isRegistering, mutateAsync: registerUser } = useMutation({
		mutationKey: [sdk.auth.register],
		mutationFn: (params: PasswordUserInput) => sdk.auth.register(params),
		onSuccess: async () => {
			await client.invalidateQueries({
				queryKey: [sdk.server.keys.claimedStatus],
				exact: false,
			})
		},
	})

	return {
		isCheckingClaimed,
		isClaimed,
		isLoggingIn,
		isPublic,
		isRegistering,
		loginError,
		loginUser,
		registerUser,
	}
}

type UseLogoutParams = {
	removeStoreUser?: () => void
}

export function useLogout({ removeStoreUser }: UseLogoutParams = {}) {
	const queryClient = useQueryClient()
	const { sdk } = useSDK()
	const { onLogout } = useClientContext()
	const { mutateAsync: logout, isPending: isLoading } = useMutation({
		mutationKey: [sdk.auth.keys.logout],
		mutationFn: () => sdk.auth.logout(),
		onSuccess: async () => {
			queryClient.clear()
			removeStoreUser?.()
			await onLogout?.()
		},
	})

	return { isLoggingOut: isLoading, logout }
}
