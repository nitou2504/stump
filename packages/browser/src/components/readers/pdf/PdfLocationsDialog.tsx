import { Dialog, Tabs, Text } from '@stump/components'
import type { PdfOutlineEntry } from '@stump/sdk'
import { Trash2 } from 'lucide-react'
import { useMemo } from 'react'

import GenericEmptyState from '@/components/GenericEmptyState'
import { useTranslate } from '@/hooks/useTranslate'

import { AnnotationsList, type ReaderAnnotation } from '../annotations'
import { flattenOutline } from './outline'
import type { PdfBookmark } from './usePdfData'

export type PdfLocationTab = 'contents' | 'bookmarks' | 'annotations'

type Props = {
	open: boolean
	onOpenChange: (open: boolean) => void
	tab: PdfLocationTab
	onTabChange: (tab: PdfLocationTab) => void
	outline: PdfOutlineEntry[]
	bookmarks: PdfBookmark[]
	annotations: ReaderAnnotation[]
	/** The PDF has no text layer, so it can't be annotated */
	hasText: boolean
	labelOf: (page: number) => string
	onGoToPage: (page: number, progression?: number) => void
	onDeleteBookmark: (id: string) => void
}

/** The book's outline, the user's bookmarks and annotations; selecting one goes there */
export default function PdfLocationsDialog({
	open,
	onOpenChange,
	tab,
	onTabChange,
	outline,
	bookmarks,
	annotations,
	hasText,
	labelOf,
	onGoToPage,
	onDeleteBookmark,
}: Props) {
	const { translate } = useTranslate()
	const flat = useMemo(() => flattenOutline(outline), [outline])

	const go = (page: number, progression?: number) => {
		onGoToPage(page, progression)
		onOpenChange(false)
	}

	const renderContents = () => {
		if (!flat.length) {
			return <GenericEmptyState title={translate('shared.pdfReader.noOutline')} />
		}
		return (
			<div className="px-2 flex flex-col">
				{flat.map((entry, index) => (
					<button
						key={`${entry.title}-${index}`}
						type="button"
						disabled={entry.page == null}
						className="gap-2 py-1.5 pr-1 flex items-center justify-between text-left hover:bg-muted disabled:opacity-60"
						style={{ paddingLeft: 4 + entry.depth * 16 }}
						onClick={() => entry.page != null && go(entry.page)}
					>
						<Text size="sm" className="line-clamp-1">
							{entry.title}
						</Text>
						{entry.page != null && (
							<Text size="xs" variant="muted" className="shrink-0">
								{labelOf(entry.page)}
							</Text>
						)}
					</button>
				))}
			</div>
		)
	}

	const renderBookmarks = () => {
		if (!bookmarks.length) {
			return <GenericEmptyState title={translate('shared.epubReader.noBookmarks')} />
		}
		return (
			<div className="px-2 flex flex-col divide-y divide-border">
				{bookmarks.map((bookmark) => (
					<div key={bookmark.id} className="gap-2 flex items-center">
						<button
							type="button"
							className="gap-1 py-1.5 px-1 min-w-0 flex flex-1 flex-col text-left hover:bg-muted"
							onClick={() =>
								go(bookmark.page, bookmark.locator?.locations?.progression ?? undefined)
							}
						>
							<Text variant="muted" size="xs" className="line-clamp-1">
								{[
									translate('shared.readerAnnotations.page', { page: labelOf(bookmark.page) }),
									bookmark.locator?.chapterTitle,
								]
									.filter(Boolean)
									.join(' · ')}
							</Text>
							{bookmark.previewContent && (
								<Text size="sm" className="line-clamp-2">
									{bookmark.previewContent}
								</Text>
							)}
						</button>
						<button
							type="button"
							className="p-2 shrink-0 rounded-md text-muted-foreground hover:bg-muted hover:text-foreground"
							title={translate('shared.readerAnnotations.removeBookmark')}
							aria-label={translate('shared.readerAnnotations.removeBookmark')}
							onClick={() => onDeleteBookmark(bookmark.id)}
						>
							<Trash2 className="h-4 w-4" />
						</button>
					</div>
				))}
			</div>
		)
	}

	const renderAnnotations = () => (
		<>
			{!hasText && (
				<Text size="sm" variant="muted" className="px-2 pb-2">
					{translate('shared.pdfReader.noTextHint')}
				</Text>
			)}
			<AnnotationsList
				annotations={annotations}
				onSelect={(annotation) => {
					const locations = annotation.locator.locations
					if (locations?.position) go(locations.position, locations.rects?.[0]?.y)
				}}
				describeLocation={(annotation) => {
					const page = annotation.locator.locations?.position
					if (!page) return null
					const label = translate('shared.readerAnnotations.page', { page: labelOf(page) })
					return annotation.locator.chapterTitle
						? `${label} · ${annotation.locator.chapterTitle}`
						: label
				}}
			/>
		</>
	)

	const tabs: Array<{ value: PdfLocationTab; label: string }> = [
		{ value: 'contents', label: translate('shared.epubReader.tableOfContents') },
		{ value: 'bookmarks', label: translate('shared.epubReader.bookmarks') },
		{ value: 'annotations', label: translate('shared.epubReader.annotations') },
	]

	return (
		<Dialog open={open} onOpenChange={onOpenChange}>
			<Dialog.Content size="md">
				<Dialog.Header>
					<Tabs value={tab} variant="primary" activeOnHover>
						<Tabs.List className="max-w-[calc(100%-2.5rem)] border-none">
							{tabs.map(({ value, label }) => (
								<Tabs.Trigger
									key={value}
									value={value}
									asChild
									className="min-w-0 sm:min-w-25"
									onClick={() => onTabChange(value)}
								>
									<Text className="cursor-pointer truncate">{label}</Text>
								</Tabs.Trigger>
							))}
						</Tabs.List>
					</Tabs>
					<Dialog.Close onClick={() => onOpenChange(false)} />
				</Dialog.Header>
				<div className="scrollbar-hide max-h-[min(60vh,420px)] min-h-[200px] overflow-y-auto">
					{tab === 'contents' && renderContents()}
					{tab === 'bookmarks' && renderBookmarks()}
					{tab === 'annotations' && renderAnnotations()}
				</div>
			</Dialog.Content>
		</Dialog>
	)
}
