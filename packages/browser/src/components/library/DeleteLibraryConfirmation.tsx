import { useGraphQLMutation } from '@stump/client'
import { TypeToConfirmModal } from '@stump/components'
import { graphql, UserPermission } from '@stump/graphql'
import { isAxiosError } from '@stump/sdk'
import { useQueryClient } from '@tanstack/react-query'
import { useCallback, useEffect, useMemo } from 'react'
import { toast } from 'sonner'

import { useAppContext } from '@/context'
import { useLeavePages } from '@/hooks/useHistoryTrail'
import { useTranslate } from '@/hooks/useTranslate'
import { usePaths } from '@/paths'

const mutation = graphql(`
	mutation DeleteLibrary($id: ID!) {
		deleteLibrary(id: $id) {
			id
		}
	}
`)

const LOCALE_KEY = 'shared.common.deleteConfirmation'
const getKey = (key: string) => `${LOCALE_KEY}.${key}`

type Props = {
	libraryId: string
	libraryName: string
	onClose: () => void
	isOpen: boolean
	trigger?: React.ReactNode
}

export default function DeleteLibraryConfirmation({
	isOpen,
	libraryId,
	libraryName,
	onClose,
	trigger,
}: Props) {
	const paths = usePaths()
	const client = useQueryClient()
	const leavePages = useLeavePages()
	const { translate } = useTranslate()

	/**
	 * The library's pages, and those of any book or series: they may belong to the library
	 * and be gone with it
	 */
	const isGonePage = useCallback(
		(path: string) => {
			const libraryPath = paths.library(libraryId)
			const appPath = path.slice(paths.home().length - 1)
			return (
				path === libraryPath ||
				path.startsWith(`${libraryPath}/`) ||
				path.startsWith(`${libraryPath}?`) ||
				/^\/(books|series)\/[^/?]/.test(appPath)
			)
		},
		[paths, libraryId],
	)

	const {
		mutate: deleteLibrary,
		isPending,
		error,
	} = useGraphQLMutation(mutation, {
		onSuccess: () => {
			onClose()
			// Pages not on screen are fetched again when shown, so they no longer list the library
			client.removeQueries({ type: 'inactive' })
			// Back to the page the user was on before the library's, as if they had pressed back
			// (the home page replaces them when there is none)
			leavePages(isGonePage, paths.home())
			// The library's pages stay on screen until that page is ready, so they are not
			// refetched: they would not find the library anymore and send the user to a 404
			client.invalidateQueries({
				predicate: (query) => !query.queryHash.includes(libraryId),
			})
		},
	})
	const { checkPermission } = useAppContext()

	const isPermitted = useMemo(
		() => checkPermission(UserPermission.DeleteLibrary),
		[checkPermission],
	)

	const handleDelete = useCallback(() => {
		if (isPermitted) {
			deleteLibrary({ id: libraryId })
		}
	}, [deleteLibrary, isPermitted, libraryId])

	useEffect(() => {
		if (!error) return

		console.error(error)
		if (isAxiosError(error)) {
			toast.error(error.message || 'An error occurred while deleting the library')
		} else {
			toast.error('An error occurred while deleting the library')
		}
	}, [error])

	const entityI18nValues = { name: libraryName, type: 'library' }

	return (
		<TypeToConfirmModal
			title={translate(getKey('title'), entityI18nValues)}
			description={translate(getKey('description'), entityI18nValues)}
			confirmText={translate(getKey('confirm'), entityI18nValues)}
			confirmVariant="destructive"
			isOpen={isOpen}
			onClose={onClose}
			onConfirm={handleDelete}
			confirmIsLoading={isPending}
			trigger={trigger}
			confirmationValue={libraryName}
			instructionText={translate(getKey('typeToConfirm'), entityI18nValues)}
		/>
	)
}
