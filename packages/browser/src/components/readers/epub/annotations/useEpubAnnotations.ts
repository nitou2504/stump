import type { ReadiumWebReaderQuery } from '@stump/graphql'
import { useMemo } from 'react'

import {
	type AnnotationCacheAdapter,
	useAnnotationMutations,
} from '../../annotations/useAnnotationMutations'
import type { EpubAnnotation } from './types'

type UseEpubAnnotationsArgs = {
	mediaId: string
	isIncognito: boolean
	initialAnnotations: EpubAnnotation[]
}

/**
 * The EPUB reader's annotations: {@link useAnnotationMutations} over the annotations its
 * `epubById` query loaded, so decorations and the annotations list update together.
 */
export function useEpubAnnotations({
	mediaId,
	isIncognito,
	initialAnnotations,
}: UseEpubAnnotationsArgs) {
	const cache = useMemo<AnnotationCacheAdapter<ReadiumWebReaderQuery>>(
		() => ({
			queryKey: ['readiumWebReader', mediaId],
			read: (data) => data?.epubById?.annotations,
			write: (data, annotations) =>
				data.epubById
					? {
							...data,
							epubById: {
								...data.epubById,
								annotations: annotations as unknown as typeof data.epubById.annotations,
							},
						}
					: data,
		}),
		[mediaId],
	)

	return useAnnotationMutations({ mediaId, isIncognito, initialAnnotations, cache })
}
