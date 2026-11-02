import { cn } from '@stump/components'
import { ArrowLeft, Bookmark, List, Search, Settings2 } from 'lucide-react'
import { Link } from 'react-router'

import { useTranslate } from '@/hooks/useTranslate'

import ControlButton from '../epub/controls/ControlButton'
import type { ReaderChrome } from './useReaderChrome'

type Props = {
	chrome: ReaderChrome
	title: string
	/** Where the back arrow goes (the book's page) */
	backTo: string
	onContents?: () => void
	/** Opens the reader's search; no button without it */
	onSearch?: () => void
	bookmark?: { active: boolean; disabled?: boolean; onToggle: () => void }
	onSettings: () => void
	className?: string
}

/**
 * The top bar every reader shares. It lies over the pages (they keep the whole screen) and
 * slides away when not needed; see `useReaderChrome` for when it shows. Only the essentials:
 * back and contents on the left, the book's title, then search, bookmark and settings.
 */
export default function ReaderTopBar({
	chrome,
	title,
	backTo,
	onContents,
	onSearch,
	bookmark,
	onSettings,
	className,
}: Props) {
	const { translate } = useTranslate()
	const { visible, barProps } = chrome

	const bookmarkLabel = translate(
		bookmark?.active
			? 'shared.readerAnnotations.removeBookmark'
			: 'shared.readerAnnotations.addBookmark',
	)

	return (
		<header
			{...barProps}
			data-testid="reader-top-bar"
			data-visible={visible}
			className={cn(
				'inset-x-0 top-0 gap-1 absolute z-40 flex items-center',
				'shadow-sm backdrop-blur-sm border-b border-border/60 bg-background/95 text-foreground',
				'ease-out transition-transform duration-200 motion-reduce:transition-none',
				visible ? 'translate-y-0' : '-translate-y-full',
				className,
			)}
			// Clear of a camera cutout or rounded corners: at the top in portrait, at a side in
			// landscape (`viewport-fit=cover` lets the reader use the whole screen)
			style={{
				height: 'calc(3rem + env(safe-area-inset-top))',
				paddingTop: 'env(safe-area-inset-top)',
				paddingLeft: 'max(0.5rem, env(safe-area-inset-left))',
				paddingRight: 'max(0.5rem, env(safe-area-inset-right))',
			}}
		>
			<Link
				to={backTo}
				title={translate('shared.imageReader.header.goToMediaOverview')}
				aria-label={translate('shared.imageReader.header.goToMediaOverview')}
			>
				<ControlButton tabIndex={-1}>
					<ArrowLeft className="h-4 w-4" />
				</ControlButton>
			</Link>
			{onContents && (
				<ControlButton
					title={translate('shared.epubReader.locationManager')}
					aria-label={translate('shared.epubReader.locationManager')}
					onClick={onContents}
				>
					<List className="h-4 w-4" />
				</ControlButton>
			)}

			<p className="px-2 text-sm min-w-0 flex-1 truncate text-center" title={title}>
				{title}
			</p>

			{onSearch && (
				<ControlButton
					title={`${translate('shared.readerAnnotations.shortcuts.search')} (Ctrl+F)`}
					aria-label={translate('shared.readerAnnotations.shortcuts.search')}
					onClick={onSearch}
				>
					<Search className="h-4 w-4" />
				</ControlButton>
			)}
			{bookmark && (
				<ControlButton
					title={`${bookmarkLabel} (B)`}
					aria-label={bookmarkLabel}
					aria-pressed={bookmark.active}
					disabled={bookmark.disabled}
					onClick={bookmark.onToggle}
				>
					<Bookmark className={cn('h-4 w-4', { 'fill-current': bookmark.active })} />
				</ControlButton>
			)}
			<ControlButton
				title={translate('shared.reader.settings.title')}
				aria-label={translate('shared.reader.settings.title')}
				onClick={onSettings}
				data-testid="reader-settings-button"
			>
				<Settings2 className="h-4 w-4" />
			</ControlButton>
		</header>
	)
}
