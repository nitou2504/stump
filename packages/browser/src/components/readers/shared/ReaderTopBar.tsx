import { cn } from '@stump/components'
import { ArrowLeft, Bookmark, List, Search } from 'lucide-react'
import { Link } from 'react-router'

import { useTranslate } from '@/hooks/useTranslate'

import ControlButton from '../epub/controls/ControlButton'
import ReaderStyleGlyph from './ReaderStyleGlyph'
import { type ReaderChrome, useDocumentFullscreen, useFullscreenKey } from './useReaderChrome'

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
	/** The reading settings are open: their button shows pressed */
	settingsOpen?: boolean
	/**
	 * Where the reader is (`23 / 315`, `Chapter 3 · 47%`), under the title; a tap on it opens
	 * "go to" (a page, a location). The way to another page whatever the reading progress is
	 * set to, even off
	 */
	position?: { text: string; label: string; onClick: () => void }
	className?: string
}

/**
 * The top bar every reader shares. It lies over the pages (they keep the whole screen) and
 * slides away when not needed; see `useReaderChrome` for when it shows. Only the essentials:
 * back and contents on the left, the book's title, then search, bookmark and the reading
 * settings (an `Aa` glyph, like other reading apps).
 */
export default function ReaderTopBar({
	chrome,
	title,
	backTo,
	onContents,
	onSearch,
	bookmark,
	onSettings,
	settingsOpen = false,
	position,
	className,
}: Props) {
	const { translate } = useTranslate()
	const { visible, barProps } = chrome
	// F11 takes the reader to the page's own full screen, which hides all of the browser's bars
	const fullscreen = useDocumentFullscreen()
	useFullscreenKey(fullscreen.toggle, fullscreen.supported)

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

			<div className="px-2 min-w-0 flex flex-1 flex-col items-center justify-center">
				<p
					className={cn('text-sm max-w-full truncate text-center', position && 'leading-[18px]')}
					title={title}
				>
					{title}
				</p>
				{position && (
					// where the reader is, as a button: tapping it opens "go to" (like the position
					// slider in the controls of Kindle, Apple Books or Mihon)
					<button
						type="button"
						onClick={position.onClick}
						title={position.label}
						aria-label={`${position.text}, ${position.label}`}
						className={cn(
							'px-1.5 rounded max-w-full truncate text-[11px] leading-[15px] text-muted-foreground tabular-nums',
							'hover:bg-foreground/7 hover:text-foreground focus-visible:ring-2 focus-visible:ring-primary focus-visible:outline-none',
							// a finger-sized hit area on touch screens, without changing how it looks
							"after:-inset-x-1 after:-inset-y-2 relative after:absolute after:content-['']",
						)}
						data-testid="reader-position"
					>
						{position.text}
					</button>
				)}
			</div>

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
				aria-haspopup="dialog"
				aria-expanded={settingsOpen}
				onClick={onSettings}
				data-testid="reader-settings-button"
				className={cn(settingsOpen && 'bg-foreground/7 text-foreground')}
			>
				<ReaderStyleGlyph className="h-5 w-5" />
			</ControlButton>
		</header>
	)
}
