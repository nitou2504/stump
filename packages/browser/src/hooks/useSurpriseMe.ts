import { useSDK } from '@stump/client'
import { graphql, MediaFilterInput, ReadingStatus } from '@stump/graphql'
import { useCallback, useState } from 'react'
import { useNavigate } from 'react-router'
import { toast } from 'sonner'

import { usePaths } from '@/paths'

const query = graphql(`
	query SurpriseMe($filter: MediaFilterInput!) {
		randomMedia(filter: $filter, count: 1) {
			id
		}
	}
`)

/** The books the user hasn't started: random picks are for finding something new */
export const UNREAD_FILTER: MediaFilterInput = { readingStatus: { is: ReadingStatus.NotStarted } }

/**
 * "Surprise me": opens a random unread book's page (a new one on every call). Shared by the
 * home's picks and the title bar's search.
 */
export function useSurpriseMe() {
	const { sdk } = useSDK()
	const paths = usePaths()
	const navigate = useNavigate()
	const [isPending, setIsPending] = useState(false)

	const surprise = useCallback(async () => {
		setIsPending(true)
		try {
			const { randomMedia } = await sdk.execute(query, {
				filter: UNREAD_FILTER,
			})
			const book = randomMedia[0]
			if (book) {
				navigate(paths.bookOverview(book.id))
			} else {
				toast.info('No unread books left')
			}
		} catch {
			toast.error("Couldn't pick a book")
		} finally {
			setIsPending(false)
		}
	}, [sdk, navigate, paths])

	return { surprise, isPending }
}
