import { cn, Text } from '@stump/components'
import { AnnotationStyle } from '@stump/graphql'
import { useMemo } from 'react'

import GenericEmptyState from '@/components/GenericEmptyState'
import { useTranslate } from '@/hooks/useTranslate'

import { annotationStyleMeta } from './styles'
import type { ReaderAnnotation } from './types'

type Props = {
	annotations: ReaderAnnotation[]
	onSelect: (annotation: ReaderAnnotation) => void
	/** The line above each quote; defaults to the chapter title, or the page for a PDF */
	describeLocation?: (annotation: ReaderAnnotation) => string | null | undefined
}

/** How the quote of each style is shown in the list, mirroring the page */
const QUOTE_CLASSES: Record<AnnotationStyle, string> = {
	[AnnotationStyle.Highlight]: 'bg-yellow-300/50 dark:bg-yellow-400/30 rounded-sm',
	[AnnotationStyle.Underline]: 'underline decoration-2 underline-offset-2',
	[AnnotationStyle.Strikethrough]: 'line-through decoration-2',
	[AnnotationStyle.Invert]: 'bg-foreground text-background rounded-sm px-0.5',
}

/**
 * A reader's annotations in reading order, each with its style, quote and note. Selecting
 * one asks the reader to go there.
 */
export default function AnnotationsList({ annotations, onSelect, describeLocation }: Props) {
	const { translate } = useTranslate()

	const sorted = useMemo(() => [...annotations].sort(compareReadingOrder), [annotations])

	if (!sorted.length) {
		return <GenericEmptyState title={translate('shared.epubReader.noAnnotations')} />
	}

	return (
		<div className="px-2 scrollbar-hide flex max-h-full flex-col divide-y divide-border overflow-y-auto">
			{sorted.map((annotation) => {
				const meta = annotationStyleMeta(annotation.style)
				const subtitle =
					describeLocation?.(annotation) ?? defaultLocationLabel(annotation, translate)
				const highlight = annotation.locator.text?.highlight

				return (
					<button
						key={annotation.id}
						data-annotation-id={annotation.id}
						className="gap-1.5 p-2 px-1 py-1.5 flex flex-col justify-start text-left hover:bg-muted"
						onClick={() => onSelect(annotation)}
					>
						<span className="gap-1.5 flex items-center">
							<meta.icon
								aria-label={translate(meta.labelKey)}
								className="h-3.5 w-3.5 shrink-0 text-muted-foreground"
							/>
							<Text variant="muted" size="xs" className="line-clamp-1">
								{subtitle}
							</Text>
						</span>
						{highlight && (
							<Text size="sm" className="line-clamp-2">
								<span className={cn('box-decoration-clone', QUOTE_CLASSES[annotation.style])}>
									{highlight}
								</span>
							</Text>
						)}
						{annotation.annotationText && (
							<Text size="sm" variant="muted" className="line-clamp-3 whitespace-pre-line">
								{annotation.annotationText}
							</Text>
						)}
					</button>
				)
			})}
		</div>
	)
}

function defaultLocationLabel(
	annotation: ReaderAnnotation,
	translate: (key: string, options?: Record<string, unknown>) => string,
) {
	const { locator } = annotation
	if (locator.chapterTitle) return locator.chapterTitle
	if (locator.type === 'application/pdf' && locator.locations?.position != null) {
		return translate('shared.readerAnnotations.page', { page: locator.locations.position })
	}
	return locator.title || locator.href
}

/** Orders by position in the book (page or total progression), oldest first on ties */
export function compareReadingOrder(a: ReaderAnnotation, b: ReaderAnnotation): number {
	const byPosition = readingPosition(a) - readingPosition(b)
	if (byPosition !== 0 && Number.isFinite(byPosition)) return byPosition
	return a.createdAt.localeCompare(b.createdAt)
}

function readingPosition(annotation: ReaderAnnotation): number {
	const locations = annotation.locator.locations
	if (locations?.totalProgression != null) return locations.totalProgression
	if (locations?.position != null) {
		// pages first, then the selection's top within the page
		return locations.position + (locations.rects?.[0]?.y ?? 0) / 2
	}
	return Number.MAX_SAFE_INTEGER
}
