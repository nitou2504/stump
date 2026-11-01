import type { Locator } from '@readium/shared'
import { useCallback, useEffect, useRef, useState } from 'react'
import { useLocation, useNavigate } from 'react-router'

/** The history state key holding how many in-book links deep an entry is */
export const LINK_DEPTH_STATE_KEY = 'epubLinkDepth'

export function linkDepthOf(state: unknown): number {
	const depth = (state as Record<string, unknown> | null)?.[LINK_DEPTH_STATE_KEY]
	return typeof depth === 'number' && depth > 0 ? depth : 0
}

type Options = {
	/** Moves the reader, resolving whether it got there */
	goTo: (locator: Locator) => Promise<boolean>
	/** Where the reader is now */
	current: () => Locator | null
}

export type LinkReturnStack = {
	/** There is a place to go back to (the Return chip shows) */
	canReturn: boolean
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
 */
export function useLinkReturnStack({ goTo, current }: Options): LinkReturnStack {
	const navigate = useNavigate()
	const location = useLocation()
	const depth = linkDepthOf(location.state)

	/** The place the reader left from each history depth */
	const [spots, setSpots] = useState<ReadonlyMap<number, Locator>>(() => new Map())
	const previousDepthRef = useRef(depth)
	const goToRef = useRef(goTo)
	useEffect(() => {
		goToRef.current = goTo
	})

	useEffect(() => {
		const previous = previousDepthRef.current
		previousDepthRef.current = depth
		if (depth >= previous) return

		const spot = spots.get(depth)
		setSpots((saved) => new Map([...saved].filter(([key]) => key < depth)))
		if (spot) void goToRef.current(spot)
	}, [depth, spots])

	const follow = useCallback(
		(to: Locator) => {
			const from = current()
			if (from) {
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
		[current, depth, location, navigate],
	)

	const canReturn = spots.has(depth - 1)
	const returnBack = useCallback(() => {
		if (canReturn) navigate(-1)
	}, [canReturn, navigate])

	return { canReturn, follow, returnBack }
}
