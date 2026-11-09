import { useSDK } from '@stump/client'
import { graphql } from '@stump/graphql'
import { useCallback, useState } from 'react'
import { useNavigate } from 'react-router'
import { toast } from 'sonner'

import { usePaths } from '@/paths'

const query = graphql(`
	query SurpriseMe {
		randomMedia(count: 1) {
			id
		}
	}
`)

/**
 * "Surprise me": opens the page of a random book the picks rules allow (a new one on every call). Shared by the
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
			// the user's picks rules apply (unread only by default)
			const { randomMedia } = await sdk.execute(query)
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
