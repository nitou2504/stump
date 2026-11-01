import type { ReaderAnnotation } from '../../annotations/types'

export type { AnnotationSelection } from '../../annotations/types'

/** An annotation in the EPUB reader (the reader-agnostic {@link ReaderAnnotation}) */
export type EpubAnnotation = ReaderAnnotation

export const ANNOTATION_DECORATION_GROUP = 'annotations'
