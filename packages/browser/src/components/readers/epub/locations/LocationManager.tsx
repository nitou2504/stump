import { Dialog, Tabs, Text } from '@stump/components'
import { useCallback, useState } from 'react'

import { useTranslate } from '@/hooks/useTranslate'

import GoToField, { resolvePlaceInput } from '../../shared/GoToField'
import { useEpubReaderContext } from '../context'
import Annotations from './Annotations'
import Bookmarks from './Bookmarks'
import TableOfContents from './TableOfContents'

type LocationTab = 'contents' | 'annotations' | 'bookmarks'

type Props = {
	open: boolean
	onOpenChange: (open: boolean) => void
	/** Opened to go to a location (`g`, the top bar's position): its field has the focus */
	focusGoTo?: boolean
}

/**
 * The book's contents, bookmarks and annotations, opened from the top bar; at the top, a
 * field to go to a location (Readium's positions, about a page each) or a share of the book
 */
export default function LocationManager({
	open: isOpen,
	onOpenChange: setIsOpen,
	focusGoTo = false,
}: Props) {
	const { translate } = useTranslate()
	const [activeTab, setActiveTab] = useState<LocationTab>('contents')
	const {
		readerMeta: { bookMeta },
		controls: { goToPosition },
	} = useEpubReaderContext()
	const totalPositions = bookMeta?.chapter.totalPositions ?? 0

	const handleClose = () => setIsOpen(false)
	const handleOpenChange = (nowOpen: boolean) => setIsOpen(nowOpen)
	const handleTabChange = (tab: LocationTab) => setActiveTab(tab)
	const handleLocationChanged = useCallback(() => {
		if (isOpen) {
			setIsOpen(false)
		}
	}, [isOpen, setIsOpen])

	const renderTabContent = () => {
		if (activeTab === 'contents') {
			return <TableOfContents onLocationChanged={handleLocationChanged} />
		} else if (activeTab === 'annotations') {
			return <Annotations onLocationChanged={handleLocationChanged} />
		} else if (activeTab === 'bookmarks') {
			return <Bookmarks onLocationChanged={handleLocationChanged} />
		}

		return null
	}

	return (
		<Dialog open={isOpen} onOpenChange={handleOpenChange}>
			<Dialog.Content size="md">
				<Dialog.Header>
					<Tabs value={activeTab} variant="primary" activeOnHover>
						{/* Leave room for the close button: on a phone the three tabs would run under it */}
						<Tabs.List className="max-w-[calc(100%-2.5rem)] border-none">
							<Tabs.Trigger
								value="contents"
								asChild
								className={TRIGGER_CLASSES}
								onClick={() => handleTabChange('contents')}
							>
								<Text className="cursor-pointer truncate">
									{translate('shared.epubReader.tableOfContents')}
								</Text>
							</Tabs.Trigger>

							<Tabs.Trigger
								value="bookmarks"
								asChild
								className={TRIGGER_CLASSES}
								onClick={() => handleTabChange('bookmarks')}
							>
								<Text className="cursor-pointer truncate">
									{translate('shared.epubReader.bookmarks')}
								</Text>
							</Tabs.Trigger>

							<Tabs.Trigger
								value="annotations"
								asChild
								className={TRIGGER_CLASSES}
								onClick={() => handleTabChange('annotations')}
							>
								<Text className="cursor-pointer truncate">
									{translate('shared.epubReader.annotations')}
								</Text>
							</Tabs.Trigger>
						</Tabs.List>
					</Tabs>

					<Dialog.Close onClick={handleClose} />
				</Dialog.Header>
				{goToPosition && totalPositions > 0 && (
					<GoToField
						label={translate('shared.reader.goToLocation')}
						placeholder={`1 – ${totalPositions}, %`}
						resolve={(input) => resolvePlaceInput(input, totalPositions)}
						invalidMessage={translate('shared.reader.noSuchLocation')}
						onGo={(position) => {
							goToPosition(position)
							handleClose()
						}}
						autoFocus={isOpen && focusGoTo}
						resetKey={isOpen}
					/>
				)}
				<div className="scrollbar-hide h-[300px] overflow-y-auto">{renderTabContent()}</div>
			</Dialog.Content>
		</Dialog>
	)
}

/** Tabs may shrink (their labels truncate) instead of keeping a 100px minimum each */
const TRIGGER_CLASSES = 'min-w-0 sm:min-w-25'
