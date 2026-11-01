import { cn, SheetPrimitive as Sheet, Text, useBoolean } from '@stump/components'
import { lazy, memo, Suspense, useId, useLayoutEffect, useMemo, useRef, useState } from 'react'

import { useTranslate } from '@/hooks/useTranslate'

import { DEBUG_ENV } from '../index.ts'

// The Markdown/HTML renderer (react-markdown, rehype-raw with parse5, ...) is a large download;
// loading it on demand keeps it out of every page's first load. Until it arrives the text is
// shown plain, so the description is readable at once and barely moves when it is formatted
const Markdown = lazy(() => import('./markdown/MarkdownPreview.tsx'))

type Props = {
	text?: string | null
	muted?: boolean
	/**
	 * - `inline` expands in place (pushes content, so not always my preference)
	 * - `sheet` opens the full text in a side sheet
	 *
	 * there are valid uses for both, e.g. if already in sheet i won't spawn another (at
	 * least not for web)
	 */
	variant?: 'inline' | 'sheet'
	/** the title of the sheet, if variant is `sheet` */
	sheetTitle?: string
}

/** The collapsed preview: about ten lines of body text */
const COLLAPSED_HEIGHT = 240
/**
 * Text taller than the preview by less than this (about two lines) is shown whole: a
 * "Read more" that would only reveal a line or two isn't worth the click
 */
const OVERFLOW_GRACE = 48

const BLOCK_END = /<\/(p|div|li|h[1-6]|blockquote)>|<br\s*\/?>/gi
const ENTITIES: Record<string, string> = {
	'&amp;': '&',
	'&lt;': '<',
	'&gt;': '>',
	'&quot;': '"',
	'&#39;': "'",
	'&nbsp;': ' ',
}

/** The description without HTML tags, for the moment before the renderer has loaded */
const toPlainText = (text: string) =>
	text
		.replace(BLOCK_END, '\n')
		.replace(/<[^>]*>/g, '')
		.replace(/&(amp|lt|gt|quot|#39|nbsp);/g, (entity) => ENTITIES[entity] ?? entity)
		.replace(/\n{3,}/g, '\n\n')
		.trim()

type MarkdownProps = { children: string; className?: string }

// Memoized so toggling expanded/collapsed doesn't re-parse (and visibly
// re-render/flicker) potentially large markdown descriptions.
const MemoizedMarkdown = memo(function LazyMarkdown({ children, className }: MarkdownProps) {
	return (
		<Suspense
			fallback={
				<div className={cn('text-foreground', className)}>
					<Text className="whitespace-pre-line">{toPlainText(children)}</Text>
				</div>
			}
		>
			<Markdown className={className}>{children}</Markdown>
		</Suspense>
	)
})

export default function ReadMore({ text, muted, variant = 'inline', sheetTitle }: Props) {
	const { translate } = useTranslate()
	const [showingAll, { toggle, on: open, off: close }] = useBoolean(false)
	const contentId = useId()

	const resolvedText = text ? text : DEBUG_ENV ? DEBUG_FAKE_TEXT : ''
	const markdownClassName = useMemo(() => cn({ 'opacity-80': muted }), [muted])

	const content = useMemo(
		() => <MemoizedMarkdown className={markdownClassName}>{resolvedText}</MemoizedMarkdown>,
		[markdownClassName, resolvedText],
	)

	// Whether the rendered text is taller than the preview, measured rather than guessed from
	// its length: the same text wraps to more or fewer lines depending on the width. The
	// measured element is never clamped, so its height is always the full text's.
	const measureRef = useRef<HTMLDivElement>(null)
	const [isOverflowing, setIsOverflowing] = useState(false)
	useLayoutEffect(() => {
		const element = measureRef.current
		if (!element) return
		const measure = () => setIsOverflowing(element.offsetHeight > COLLAPSED_HEIGHT + OVERFLOW_GRACE)
		measure()
		if (typeof ResizeObserver === 'undefined') return
		const observer = new ResizeObserver(measure)
		observer.observe(element)
		return () => observer.disconnect()
	}, [content])

	if (!resolvedText && !DEBUG_ENV) {
		return null
	}

	const isExpandedInline = variant === 'inline' && showingAll
	const isClamped = isOverflowing && !isExpandedInline

	return (
		<div>
			<div
				id={contentId}
				className={cn(isClamped && 'overflow-hidden')}
				style={
					isClamped
						? { maxHeight: COLLAPSED_HEIGHT, overflowAnchor: 'none' }
						: { overflowAnchor: 'none' }
				}
			>
				<div ref={measureRef}>{content}</div>
			</div>

			{isClamped && (
				<div
					aria-hidden
					className="-mt-8 h-8 pointer-events-none bg-linear-to-t from-background to-transparent"
				/>
			)}

			{/* Only when there is more to show: text that fits the preview gets no toggle at all */}
			{isOverflowing && (
				<div className="mt-2 relative flex w-full items-center">
					<div className="flex-1 border-t border-dashed border-border" />
					<button
						type="button"
						onClick={toggle}
						aria-expanded={showingAll}
						aria-controls={variant === 'inline' ? contentId : undefined}
						className="px-3 py-0.5 text-xs font-medium cursor-pointer rounded-full border border-dashed border-border bg-background text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
					>
						{isExpandedInline
							? translate('shared.common.readLess')
							: translate('shared.common.readMore')}
					</button>
					<div className="flex-1 border-t border-dashed border-border" />
				</div>
			)}

			{variant === 'sheet' && isOverflowing && (
				<Sheet open={showingAll} onOpenChange={(isOpen) => (isOpen ? open() : close())}>
					<Sheet.Content className="overflow-y-auto" closeIcon size="lg">
						<Sheet.Header>
							<Sheet.Title className="pr-8">{sheetTitle}</Sheet.Title>
						</Sheet.Header>
						<div className="px-4 pb-6 text-sm">{content}</div>
					</Sheet.Content>
				</Sheet>
			)}
		</div>
	)
}

const DEBUG_FAKE_TEXT =
	'Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed varius semper dolor, eget egestas velit porta ut. \
	Integer blandit lectus nisi, a suscipit eros malesuada eu. Praesent vel sodales ipsum, ut porttitor erat. Aliquam faucibus erat a ante \
	consectetur imperdiet. Curabitur in est ac nisi feugiat facilisis a in nisi. Ut auctor rutrum nibh a tincidunt. Proin non hendrerit risus, \
	sagittis malesuada odio. Phasellus condimentum hendrerit libero nec ultrices.\
	Praesent lacinia, magna vel sodales tempus, tellus metus ultricies odio, non porttitor lectus tortor ac ante. \
	Nullam malesuada nec massa eget facilisis. Aenean in nisi lacus. Etiam et tortor vel lacus maximus imperdiet. Fusce \
	scelerisque dapibus fermentum. Nunc non mauris rhoncus neque tincidunt convallis id et nisl. Donec lobortis at lectus quis venenatis. \
	Ut lacus urna, accumsan sed nisl eget, auctor auctor massa. Duis scelerisque aliquam scelerisque. In hac habitasse platea dictumst. Suspendisse \
	consequat nisi nec enim finibus, sit amet gravida sem ultrices. Vestibulum feugiat erat et tincidunt pellentesque. Sed interdum mi ac quam convallis lobortis.'
