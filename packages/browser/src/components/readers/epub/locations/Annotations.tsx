import { useCallback } from 'react'

import { AnnotationsList, type ReaderAnnotation } from '../../annotations'
import { useEpubReaderContext } from '../context'

type Props = {
	onLocationChanged?: () => void
}

/** The EPUB's annotations in the location manager; selecting one goes to it */
export default function Annotations({ onLocationChanged }: Props) {
	const {
		readerMeta: { bookMeta },
		controls: { onGoToLocator },
	} = useEpubReaderContext()

	const handleSelect = useCallback(
		async (annotation: ReaderAnnotation) => {
			const navigated = await onGoToLocator(annotation.locator)
			if (navigated) {
				onLocationChanged?.()
			}
		},
		[onGoToLocator, onLocationChanged],
	)

	return (
		<AnnotationsList
			annotations={bookMeta?.annotations ?? []}
			onSelect={(annotation) => void handleSelect(annotation)}
		/>
	)
}
