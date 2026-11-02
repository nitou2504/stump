import { cn } from '@stump/components'
import { useEffect, useLayoutEffect, useRef, useState } from 'react'

import { chapterAround, thinTicks } from './readingProgress'

type Props = {
	/** Where the chapters start in the book, from 0 to 1 (no ticks when unknown) */
	chapters: number[]
	/** Where the reader is in the book, from 0 to 1 */
	position: number
	/** The chapter's name, first (it's cut short before the numbers are) */
	left?: string
	/** The numbers after it, in their order (all joined by middle dots) */
	right: string[]
	/** A right-to-left book: the rule fills from the right */
	rtl?: boolean
	/** Opens the contents (EPUB), go to page (PDF) or the pages (comics) */
	onClick: () => void
	/** The whole bar read out as one button, e.g. "Chapter 1, 12 pages left in chapter, 47%" */
	label: string
	/** Hidden with the top bar (`With controls`), or while the settings sheet covers it */
	visible: boolean
	/** The top bar shows: the words move to full ink */
	emphasized?: boolean
	/**
	 * A paper-coloured band under the bar, for pages that scroll or sit under it (PDF, comics,
	 * EPUB scrolling): the bar never sits on the text itself
	 */
	band?: boolean
	/** Placed on the screen (`fixed`) rather than in the reader's frame (`absolute`) */
	fixed?: boolean
	/** The bar's horizontal place, e.g. aligned to the EPUB's text column */
	style?: React.CSSProperties
	className?: string
}

/**
 * The reading bar at the foot of the page, like KOReader's footer: a hairline rule across
 * the text column with a tick at each chapter start, the current chapter drawn thicker and
 * the position as an ink notch; under it one centred line, the chapter and the numbers
 * joined by middle dots (on a narrow screen the chapter is cut short first). A tap on it
 * opens "go to" and never turns the page under it.
 */
export default function ReadingBar({
	chapters,
	position,
	left,
	right,
	rtl = false,
	onClick,
	label,
	visible,
	emphasized = false,
	band = false,
	fixed = false,
	style,
	className,
}: Props) {
	const ruleRef = useRef<HTMLDivElement>(null)
	const [width, setWidth] = useState(0)
	useLayoutEffect(() => {
		const element = ruleRef.current
		if (!element) return undefined
		setWidth(element.clientWidth)
		if (typeof ResizeObserver === 'undefined') return undefined
		const observer = new ResizeObserver(() => setWidth(element.clientWidth))
		observer.observe(element)
		return () => observer.disconnect()
	}, [])

	const at = Math.min(1, Math.max(0, Number.isFinite(position) ? position : 0))
	const ticks = thinTicks(chapters, width)
	const [chapterFrom, chapterTo] = chapters.length ? chapterAround(chapters, at) : [0, 0]
	const percent = (value: number) => `${(value * 100).toFixed(3)}%`
	const stop = (event: React.SyntheticEvent) => event.stopPropagation()

	return (
		<>
			{band && (
				<div
					aria-hidden
					className={cn(
						'inset-x-0 bottom-0 pointer-events-none z-29 bg-linear-to-b from-transparent via-background via-45% to-background transition-opacity duration-200 motion-reduce:transition-none',
						fixed ? 'fixed' : 'absolute',
						visible ? 'opacity-100' : 'opacity-0',
					)}
					style={{ height: 'calc(52px + env(safe-area-inset-bottom))' }}
				/>
			)}
			<button
				type="button"
				onClick={(event) => {
					event.stopPropagation()
					onClick()
				}}
				onPointerDown={stop}
				onPointerUp={stop}
				onTouchStart={stop}
				onTouchEnd={stop}
				aria-label={label}
				title={label}
				tabIndex={visible ? 0 : -1}
				aria-hidden={!visible}
				data-testid="reading-bar"
				className={cn(
					'pt-3 pb-1 group z-30 block text-center select-none',
					fixed ? 'fixed' : 'absolute',
					'ease-out transition-[opacity,transform] duration-200 motion-reduce:transition-none',
					'focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-offset-2 focus-visible:ring-offset-background focus-visible:outline-none',
					visible ? 'translate-y-0 opacity-100' : 'translate-y-2 pointer-events-none opacity-0',
					className,
				)}
				// the words 14px above the screen's foot, or above the home indicator on a phone
				style={{ bottom: `max(10px, calc(6px + env(safe-area-inset-bottom)))`, ...style }}
			>
				<div
					ref={ruleRef}
					aria-hidden
					className={cn('relative h-[9px]', rtl && '-scale-x-100')}
					data-testid="reading-bar-rule"
				>
					{/* the hairline: the read part in muted ink, the rest in the border colour */}
					<span className="inset-x-0 top-1 absolute h-px bg-border" />
					<span
						className="top-1 left-0 absolute h-px bg-muted-foreground"
						style={{ width: percent(at) }}
					/>
					{chapters.length > 0 && chapterTo > chapterFrom && (
						// the current chapter, thicker: what's left of it is the unread part
						<span
							className="absolute top-[3px] h-[3px] overflow-hidden rounded-[2px] bg-border"
							style={{
								left: `calc(${percent(chapterFrom)} + 2px)`,
								width: `max(0px, calc(${percent(chapterTo - chapterFrom)} - 4px))`,
							}}
							data-testid="reading-bar-chapter"
						>
							<span
								className="inset-y-0 left-0 absolute bg-muted-foreground"
								style={{
									width: percent(
										Math.min(1, Math.max(0, (at - chapterFrom) / (chapterTo - chapterFrom))),
									),
								}}
							/>
						</span>
					)}
					{ticks.map((tick) => (
						<span
							key={tick}
							className="h-1.5 absolute top-[1.5px] w-px bg-muted-foreground opacity-75"
							style={{ left: percent(tick) }}
							data-testid="reading-bar-tick"
						/>
					))}
					{/* the position: an ink notch */}
					<span
						className="w-0.5 absolute -top-px h-[11px] -translate-x-px rounded-[1px] bg-foreground"
						style={{ left: percent(at) }}
					/>
				</div>
				<div
					className={cn(
						'leading-4 font-medium min-w-0 mt-[7px] flex justify-center text-[11.5px] tracking-[0.005em] whitespace-nowrap tabular-nums transition-colors',
						emphasized ? 'text-foreground' : 'text-muted-foreground',
						'group-hover:text-foreground',
					)}
					data-testid="reading-bar-text"
				>
					{left && (
						// cut short well before the numbers are
						<span className="min-w-0 [flex-shrink:50] truncate" data-testid="reading-bar-left">
							{left}
						</span>
					)}
					{left && right.length > 0 && <Dot />}
					{right.length > 0 && (
						<span className="min-w-0 truncate" data-testid="reading-bar-right">
							{right.map((item, index) => (
								<span key={item + index}>
									{index > 0 && <Dot />}
									{item}
								</span>
							))}
						</span>
					)}
				</div>
			</button>
		</>
	)
}

/** The middle dot between the bar's items */
function Dot() {
	return (
		<span aria-hidden className="shrink-0 px-[5px] opacity-55">
			·
		</span>
	)
}

/**
 * The height the bar takes at the foot of the page, which pages that turn leave free so it
 * never covers their text (the rule and the words, their gaps, and the bar's distance from
 * the screen's foot)
 */
export const READING_BAR_ZONE = 'calc(52px + env(safe-area-inset-bottom))'

/** The local time for the bar ("21:06" or "9:06 PM", per the locale), updated every minute */
export function useClockText(enabled: boolean): string {
	const format = () =>
		new Date().toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' })
	const [text, setText] = useState(format)
	useEffect(() => {
		if (!enabled) return undefined
		setText(format())
		let interval: number | undefined
		// on the minute, then every minute
		const timeout = window.setTimeout(
			() => {
				setText(format())
				interval = window.setInterval(() => setText(format()), 60_000)
			},
			60_000 - (Date.now() % 60_000),
		)
		return () => {
			window.clearTimeout(timeout)
			window.clearInterval(interval)
		}
	}, [enabled])
	return text
}
