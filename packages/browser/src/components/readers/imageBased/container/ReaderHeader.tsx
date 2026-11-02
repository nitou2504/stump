import { PDF_EXTENSION } from '@stump/client'
import { cn, Link, Text } from '@stump/components'
import { motion } from 'framer-motion'
import { ArrowLeft, FileText, Fullscreen, Shrink } from 'lucide-react'
import { useFullscreen } from 'rooks'

import { useTranslate } from '@/hooks/useTranslate'
import { usePaths } from '@/paths'
import { useBookPreferences } from '@/scenes/book/reader/useBookPreferences'
import { useAppStore } from '@/stores'

import { usePdfReaderStore } from '../../pdf/store'
import { useImageBaseReaderContext } from '../context'
import ControlButton from './ControlButton'
import SettingsDialog from './SettingsDialog'
import TimerMenu from './TimerMenu'

export default function ReaderHeader() {
	const { translate } = useTranslate()
	const { book } = useImageBaseReaderContext()
	const {
		settings: { showToolBar },
	} = useBookPreferences({ book })
	const paths = usePaths()

	const platform = useAppStore((store) => store.platform)

	const { id, resolvedName } = book
	// optional: tests and older callers may pass a book without its extension
	const isPdf = !!book.extension?.match(PDF_EXTENSION)
	const setPdfPreferences = usePdfReaderStore((state) => state.setBookPreferences)

	const { isFullscreenAvailable, isFullscreenEnabled, toggleFullscreen } = useFullscreen()

	const FullScreenIcon = isFullscreenEnabled ? Shrink : Fullscreen

	return (
		<motion.nav
			// @ts-expect-error: It does have className?
			className={cn('left-0 top-0 h-12 px-4 fixed z-100 flex w-full items-center text-foreground', {
				'top-9': platform !== 'browser',
			})}
			initial={false}
			animate={showToolBar ? 'visible' : 'hidden'}
			variants={transition}
			transition={{ duration: 0.2, ease: 'easeInOut' }}
		>
			<div className="flex w-full items-center justify-between">
				<div className="space-x-4 flex items-center">
					<Link
						className="flex items-center text-foreground hover:text-foreground/80"
						title={translate('shared.imageReader.header.goToMediaOverview')}
						to={paths.bookOverview(id)}
					>
						<ArrowLeft size={'1.25rem'} />
					</Link>
				</div>

				<Text className="text-foreground">{resolvedName}</Text>

				<div className="space-x-1.5 flex items-center">
					{isPdf && (
						<ControlButton
							title={translate('shared.pdfReader.openAsText')}
							aria-label={translate('shared.pdfReader.openAsText')}
							onClick={() => setPdfPreferences(id, { reader: 'pdf' })}
						>
							<FileText className="h-4 w-4" />
						</ControlButton>
					)}

					{isFullscreenAvailable && (
						<ControlButton onClick={toggleFullscreen}>
							<FullScreenIcon className="h-4 w-4" />
						</ControlButton>
					)}

					<TimerMenu />

					<SettingsDialog />
				</div>
			</div>
		</motion.nav>
	)
}

const transition = {
	hidden: {
		opacity: 0,
		transition: {
			duration: 0.2,
			ease: 'easeInOut',
		},
		y: '-100%',
	},
	visible: {
		opacity: 1,
		transition: {
			duration: 0.2,
			ease: 'easeInOut',
		},
		y: 0,
	},
}
