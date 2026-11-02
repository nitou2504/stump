import { type RefObject, useLayoutEffect, useState } from 'react'

export type ElementSize = { width: number; height: number }

/** The content-box size of an element, kept up to date with a ResizeObserver */
export function useElementSize(ref: RefObject<HTMLElement | null>): ElementSize {
	const [size, setSize] = useState<ElementSize>({ width: 0, height: 0 })

	useLayoutEffect(() => {
		const element = ref.current
		if (!element) return undefined

		const update = (width: number, height: number) => {
			const rounded = { width: Math.floor(width), height: Math.floor(height) }
			setSize((previous) =>
				previous.width === rounded.width && previous.height === rounded.height ? previous : rounded,
			)
		}
		update(element.clientWidth, element.clientHeight)

		if (typeof ResizeObserver === 'undefined') return undefined
		const observer = new ResizeObserver(() => update(element.clientWidth, element.clientHeight))
		observer.observe(element)
		return () => observer.disconnect()
	}, [ref])

	return size
}
