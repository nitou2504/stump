import type { Locator } from '@readium/shared'
import { useCallback, useEffect, useRef, useState } from 'react'
import { useLocation, useNavigate } from 'react-router'

/** The history state key holding how many in-book links deep an entry is */
export const LINK_DEPTH_STATE_KEY = 'epubLinkDepth'

export function linkDepthOf(state: unknown): number {
	const depth = (state as Record<string, unknown> | null)?.[LINK_DEPTH_STATE_KEY]
	return typeof depth === 'number' && depth > 0 ? depth : 0
}

/** The reader's own back and forward (see `shared/navHistory`), told about links too */
export type LinkHistory = {
	/** A link is about to move the reader away from `from` */
	followed: (from: Locator) => void
	/** Back took the reader from `from` to `spot`, the place a link was followed from */
	returned: (spot: Locator, from: Locator | null) => void
}

type Options = {
	/** Moves the reader, resolving whether it got there */
	goTo: (locator: Locator) => Promise<boolean>
	/** Where the reader is now */
	current: () => Locator | null
	/** The reader's back and forward, so a followed link is one of its places */
	history?: LinkHistory
}

export type LinkReturnStack = {
	/** There is a place to go back to (the Return chip shows) */
	canReturn: boolean
	/** Where Return (or the browser's Back) leads: the place the last link was followed from */
	returnSpot: Locator | null
	/** Follow an in-book link: remember the current place, then go to `to` */
	follow: (to: Locator) => Promise<boolean>
	/** Go back to where the last followed link was (the same as the browser's Back) */
	returnBack: () => void
}

/**
 * Back for in-book links. Following a link adds a browser history entry (the same page,
 * with its depth in the history state) and remembers where the reader was. Going back,
 * with the Return chip or the browser's Back, lands on the previous entry, and the reader
 * returns to the place remembered for it. Forward doesn't jump again.
 *
 * The followed link is also one of the reader's places to go back to (`history`): the
 * contents' "Back to" and the Return chip are the same step.
 */
export function useLinkReturnStack({ goTo, current, history }: Options): LinkReturnStack {
	const navigate = useNavigate()
	const location = useLocation()
	const depth = linkDepthOf(location.state)

	/** The place the reader left from each history depth */
	const [spots, setSpots] = useState<ReadonlyMap<number, Locator>>(() => new Map())
	const previousDepthRef = useRef(depth)
	const goToRef = useRef(goTo)
	const currentRef = useRef(current)
	const historyRef = useRef(history)
	useEffect(() => {
		goToRef.current = goTo
		currentRef.current = current
		historyRef.current = history
	})

	useEffect(() => {
		const previous = previousDepthRef.current
		previousDepthRef.current = depth
		if (depth >= previous) return

		const spot = spots.get(depth)
		setSpots((saved) => new Map([...saved].filter(([key]) => key < depth)))
		if (spot) {
			historyRef.current?.returned(spot, currentRef.current())
			void goToRef.current(spot)
		}
	}, [depth, spots])

	const follow = useCallback(
		(to: Locator) => {
			const from = current()
			if (from) {
				history?.followed(from)
				setSpots((saved) => new Map(saved).set(depth, from))
				navigate(
					{ pathname: location.pathname, search: location.search, hash: location.hash },
					{
						state: {
							...((location.state as Record<string, unknown> | null) ?? {}),
							[LINK_DEPTH_STATE_KEY]: depth + 1,
						},
					},
				)
			}
			return goToRef.current(to)
		},
		[current, depth, history, location, navigate],
	)

	const returnSpot = spots.get(depth - 1) ?? null
	const canReturn = returnSpot != null
	const returnBack = useCallback(() => {
		if (canReturn) navigate(-1)
	}, [canReturn, navigate])

	return { canReturn, returnSpot, follow, returnBack }
}
