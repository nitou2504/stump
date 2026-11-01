import { Column } from '@tanstack/react-table'
import { CSSProperties, useEffect, useState } from 'react'

/**
 * Entity tables lay their columns out as flex rows, so every table fills the width it has:
 *
 * - a column with `meta.isGrow` (names, people, genres, ...) is flexible: it is never
 *   narrower than its `size`, and shares the width left by the other columns in proportion
 *   to its `size`
 * - any other column (cover, numbers, dates, ...) keeps its `size`
 *
 * Cells stay on a single line (long values are truncated, with the full value on hover).
 * When the table needs more than the available width, it scrolls sideways instead of
 * wrapping into tall rows (see `useTableScroll`).
 */
export function columnStyle<T>(column: Column<T, unknown>): CSSProperties {
	const size = column.getSize()
	if (column.columnDef.meta?.isGrow) {
		return { flex: `${size} 1 ${size}px`, minWidth: size }
	}
	return { flex: `0 0 ${size}px`, width: size, minWidth: size }
}

/** The width a table needs before its flexible columns would get narrower than their size */
export function tableMinWidth<T>(columns: Column<T, unknown>[]) {
	return columns.reduce((total, column) => total + column.getSize(), 0)
}

/**
 * Whether a table wider than its container should scroll sideways. Only then is its
 * wrapper made a scroll container: a horizontally scrolling wrapper would otherwise stop
 * the sticky header from following the page's vertical scroll.
 *
 * Attach `ref` to the table's wrapper.
 */
export function useTableScroll(minWidth: number) {
	const [element, setElement] = useState<HTMLElement | null>(null)
	const [width, setWidth] = useState(0)

	useEffect(() => {
		if (!element) return
		// called once right away, then on every resize
		const observer = new ResizeObserver(([entry]) => {
			if (entry) setWidth(entry.contentRect.width)
		})
		observer.observe(element)
		return () => observer.disconnect()
	}, [element])

	return { ref: setElement, scrolls: width > 0 && width < minWidth }
}
