import { cx } from '@stump/components'
import { Bookmark } from 'lucide-react'

import { useTranslate } from '@/hooks/useTranslate'

import ControlButton from './ControlButton'
import { useEpubBookmark } from './useEpubBookmark'

export default function BookmarkToggle() {
	const { translate } = useTranslate()
	const {
		createBookmark,
		deleteBookmark,
		isUnknownCfiRange,
		canBookmarkCurrent,
		currentIsBookmarked,
	} = useEpubBookmark()

	const handleClick = async () => {
		if (currentIsBookmarked) {
			deleteBookmark()
		} else if (canBookmarkCurrent) {
			createBookmark()
		}
	}

	const isDisabled = isUnknownCfiRange

	return (
		<ControlButton
			disabled={isDisabled}
			onClick={handleClick}
			title={`${translate(
				currentIsBookmarked
					? 'shared.readerAnnotations.removeBookmark'
					: 'shared.readerAnnotations.addBookmark',
			)} (B)`}
			aria-pressed={currentIsBookmarked}
		>
			<Bookmark className={cx('h-4 w-4', { 'fill-current': currentIsBookmarked })} />
		</ControlButton>
	)
}
