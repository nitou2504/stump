import { Dialog, Tabs, Text } from '@stump/components'
import { List } from 'lucide-react'
import { useCallback, useState } from 'react'

import { useTranslate } from '@/hooks/useTranslate'

import ControlButton from '../controls/ControlButton'
import Annotations from './Annotations'
import Bookmarks from './Bookmarks'
import TableOfContents from './TableOfContents'

type LocationTab = 'contents' | 'annotations' | 'bookmarks'

export default function LocationManager() {
	const { translate } = useTranslate()
	const [isOpen, setIsOpen] = useState(false)
	const [activeTab, setActiveTab] = useState<LocationTab>('contents')

	const handleClose = () => setIsOpen(false)
	const handleOpenChange = (nowOpen: boolean) => {
		if (nowOpen) {
			setIsOpen(true)
		} else {
			handleClose()
		}
	}
	const handleTabChange = (tab: LocationTab) => setActiveTab(tab)
	const handleLocationChanged = useCallback(() => {
		if (isOpen) {
			setIsOpen(false)
		}
	}, [isOpen])

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
			<Dialog.Trigger asChild>
				<ControlButton title={translate('shared.epubReader.locationManager')}>
					<List className="h-4 w-4" />
				</ControlButton>
			</Dialog.Trigger>
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
				<div className="scrollbar-hide h-[300px] overflow-y-auto">{renderTabContent()}</div>
			</Dialog.Content>
		</Dialog>
	)
}

/** Tabs may shrink (their labels truncate) instead of keeping a 100px minimum each */
const TRIGGER_CLASSES = 'min-w-0 sm:min-w-25'
