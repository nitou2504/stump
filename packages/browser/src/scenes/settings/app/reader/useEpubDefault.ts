import type { BookPreferences } from '@stump/client'
import { useCallback } from 'react'
import { useShallow } from 'zustand/react/shallow'

import { useReaderStore } from '@/stores'

type EpubDefaultKey = 'fontFamily' | 'fontSize' | 'lineHeight'

/**
 * One of every EPUB's defaults, as the reader uses it: the default picked for every EPUB
 * (the reading panel's "Make default", or this screen), else the reader's own setting. Set
 * here, it's every EPUB's default, the same one "Make default" sets.
 */
export function useEpubDefault<K extends EpubDefaultKey>(
	key: K,
): [BookPreferences[K] | undefined, (value: BookPreferences[K] | undefined) => void] {
	const { epubDefault, setting, setFormatDefaults } = useReaderStore(
		useShallow((state) => ({
			epubDefault: state.formatDefaults?.epub?.[key],
			setting: state.settings[key],
			setFormatDefaults: state.setFormatDefaults,
		})),
	)
	const set = useCallback(
		(value: BookPreferences[K] | undefined) =>
			setFormatDefaults('epub', { [key]: value } as Partial<BookPreferences>),
		[key, setFormatDefaults],
	)
	return [epubDefault !== undefined ? epubDefault : setting, set]
}
