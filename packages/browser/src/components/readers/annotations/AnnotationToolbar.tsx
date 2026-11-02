import { cn } from '@stump/components'
import type { AnnotationStyle } from '@stump/graphql'
import { Copy, NotebookPen, Trash2 } from 'lucide-react'
import { useLayoutEffect, useRef, useState } from 'react'

import { useTranslate } from '@/hooks/useTranslate'

import { COPY_SHORTCUT } from '../shared/readerKeys'
import { ANNOTATION_STYLES, type AnnotationStyleMeta, NOTE_SHORTCUT } from './styles'
import type { ToolbarAnchor } from './types'

type Size = { width: number; height: number }
type Viewport = { width: number; height: number }

const GAP_PX = 8
const MARGIN_PX = 8

/**
 * Where the floating toolbar goes for an anchor (the selected text): centred above it, or
 * below it when there is no room above (or when `preferBelow`, as on touch screens where
 * the system's own copy/paste bubble sits above the selection). Always kept inside the
 * viewport.
 */
export function computeToolbarPosition(
	anchor: ToolbarAnchor,
	size: Size,
	viewport: Viewport,
	{ preferBelow = false }: { preferBelow?: boolean } = {},
): { left: number; top: number; placement: 'above' | 'below' } {
	const maxLeft = Math.max(MARGIN_PX, viewport.width - size.width - MARGIN_PX)
	const left = clamp(anchor.x + anchor.width / 2 - size.width / 2, MARGIN_PX, maxLeft)

	const above = anchor.y - GAP_PX - size.height
	const below = anchor.y + anchor.height + GAP_PX
	const fitsAbove = above >= MARGIN_PX
	const fitsBelow = below + size.height <= viewport.height - MARGIN_PX

	const placement: 'above' | 'below' = preferBelow
		? fitsBelow || !fitsAbove
			? 'below'
			: 'above'
		: fitsAbove || !fitsBelow
			? 'above'
			: 'below'

	const maxTop = Math.max(MARGIN_PX, viewport.height - size.height - MARGIN_PX)
	const top = clamp(placement === 'above' ? above : below, MARGIN_PX, maxTop)

	return { left, top, placement }
}

function clamp(value: number, min: number, max: number) {
	return Math.min(max, Math.max(min, value))
}

/**
 * On touch screens the toolbar doesn't float next to the selection, where it would cover the
 * selection's handles and the system's own copy/share menu: it docks to the bottom edge, or
 * to the top (below the reader's top bar) when the selection is in the lower half
 */
export function dockedEdge(anchor: ToolbarAnchor, viewportHeight: number): 'top' | 'bottom' {
	return anchor.y + anchor.height / 2 > viewportHeight / 2 ? 'top' : 'bottom'
}

const isCoarsePointer = () =>
	typeof window !== 'undefined' && !!window.matchMedia?.('(pointer: coarse)').matches

type Props = {
	/** The selected text or annotation, in viewport coordinates */
	anchor: ToolbarAnchor
	/**
	 * - `selection`: new text. Every style is one tap, then Note and Copy
	 * - `annotation`: an existing annotation. The same, with its style pressed, and Delete
	 */
	mode: 'selection' | 'annotation'
	/** The existing annotation's style */
	activeStyle?: AnnotationStyle
	/** Whether the existing annotation has a note (the Note button then edits it) */
	hasNote?: boolean
	onStyle: (style: AnnotationStyle) => void
	onNote: () => void
	/** Copies the selected (or annotated) text; no Copy button without it */
	onCopy?: () => void
	onDelete?: () => void
	className?: string
}

/**
 * The compact floating toolbar for annotating text, shared by the readers:
 * `[highlight][underline][strikethrough][invert] | [note] | [copy]` (and delete for an
 * existing annotation). Glyph buttons with tooltips that name their keyboard shortcut.
 * Pressing a button doesn't take the focus, so the reader's text selection survives.
 */
export default function AnnotationToolbar({
	anchor,
	mode,
	activeStyle,
	hasNote,
	onStyle,
	onNote,
	onCopy,
	onDelete,
	className,
}: Props) {
	const { translate } = useTranslate()
	const ref = useRef<HTMLDivElement>(null)
	const [size, setSize] = useState<Size | null>(null)

	// Measure before paint so the toolbar never flashes off-screen
	useLayoutEffect(() => {
		const element = ref.current
		if (!element) return
		const { width, height } = element.getBoundingClientRect()
		setSize((previous) =>
			previous?.width === width && previous?.height === height ? previous : { width, height },
		)
	}, [mode, anchor.width, anchor.height])

	const docked = isCoarsePointer()
	const position = computeToolbarPosition(anchor, size ?? { width: 0, height: 0 }, {
		width: window.innerWidth,
		height: window.innerHeight,
	})
	const edge = dockedEdge(anchor, window.innerHeight)
	const placementStyle: React.CSSProperties = docked
		? {
				left: '50%',
				transform: 'translateX(-50%)',
				...(edge === 'bottom'
					? { bottom: 'calc(env(safe-area-inset-bottom) + 30px)' }
					: { top: 'calc(env(safe-area-inset-top) + 56px)' }),
			}
		: { left: position.left, top: position.top }

	const noteLabel = translate(
		hasNote ? 'shared.readerAnnotations.editNote' : 'shared.readerAnnotations.addNote',
	)

	const renderStyleButton = (meta: AnnotationStyleMeta) => {
		const label = translate(meta.labelKey)
		const isActive = mode === 'annotation' && meta.style === activeStyle
		return (
			<ToolbarButton
				key={meta.style}
				label={label}
				shortcut={meta.shortcut}
				pressed={mode === 'annotation' ? isActive : undefined}
				onClick={() => onStyle(meta.style)}
				data-style={meta.style}
			>
				<meta.icon className="h-4 w-4 shrink-0" />
			</ToolbarButton>
		)
	}

	return (
		<div
			ref={ref}
			role="toolbar"
			aria-label={translate('shared.readerAnnotations.toolbar')}
			data-testid="annotation-toolbar"
			data-docked={docked ? edge : undefined}
			className={cn(
				'gap-0.5 p-1 shadow-lg fixed z-50 flex items-center rounded-lg border border-border bg-background text-foreground',
				{ invisible: !size && !docked },
				className,
			)}
			style={placementStyle}
			// Keep the focus (and so the text selection) where it is: on touch screens too, where
			// a tap would otherwise collapse the selection before the button reads it
			onMouseDown={(event) => event.preventDefault()}
			onPointerDown={(event) => event.preventDefault()}
		>
			{ANNOTATION_STYLES.map((meta) => renderStyleButton(meta))}
			<Divider />
			<ToolbarButton label={noteLabel} shortcut={NOTE_SHORTCUT} onClick={onNote}>
				<NotebookPen className="h-4 w-4 shrink-0" />
			</ToolbarButton>
			{onCopy && (
				<>
					<Divider />
					<ToolbarButton
						label={translate('shared.readerAnnotations.copy')}
						shortcut={COPY_SHORTCUT}
						onClick={onCopy}
					>
						<Copy className="h-4 w-4 shrink-0" />
					</ToolbarButton>
				</>
			)}
			{mode === 'annotation' && onDelete && (
				<ToolbarButton
					label={translate('shared.readerAnnotations.delete')}
					shortcut="Del"
					onClick={onDelete}
					className="text-red-600 dark:text-red-400"
				>
					<Trash2 className="h-4 w-4 shrink-0" />
				</ToolbarButton>
			)}
		</div>
	)
}

function Divider() {
	return <span aria-hidden className="mx-0.5 h-5 w-px shrink-0 bg-border" />
}

type ToolbarButtonProps = {
	label: string
	shortcut?: string
	pressed?: boolean
	onClick: () => void
	children: React.ReactNode
	className?: string
} & Omit<React.ButtonHTMLAttributes<HTMLButtonElement>, 'onClick' | 'children'>

function ToolbarButton({
	label,
	shortcut,
	pressed,
	onClick,
	children,
	className,
	...rest
}: ToolbarButtonProps) {
	const title = shortcut
		? `${label} (${shortcut.length === 1 ? shortcut.toUpperCase() : shortcut})`
		: label
	return (
		<button
			type="button"
			title={title}
			aria-label={label}
			aria-pressed={pressed}
			aria-keyshortcuts={shortcut}
			onClick={onClick}
			className={cn(
				'h-10 w-10 md:h-8 md:w-8 flex shrink-0 items-center justify-center rounded-md',
				'hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none',
				{ 'bg-muted ring-1 ring-border': pressed },
				className,
			)}
			{...rest}
		>
			{children}
		</button>
	)
}
