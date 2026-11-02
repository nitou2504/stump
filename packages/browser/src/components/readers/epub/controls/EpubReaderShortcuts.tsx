import { useEffect, useRef } from 'react'
import { toast } from 'sonner'

import { useTheme } from '@/hooks/useTheme'
import { useTranslate } from '@/hooks/useTranslate'
import { useReaderStore } from '@/stores'

import {
	isEditableTarget,
	type ReaderShortcutAction,
	resolveReaderShortcut,
} from '../../annotations/shortcuts'
import { nextDayNightTheme } from '../../shared/readerKeys'
import { useEpubReaderControls } from '../context'
import { useEpubBookmark } from './useEpubBookmark'

export type AnnotationShortcutAction = Exclude<
	ReaderShortcutAction,
	{ type: 'bookmark' | 'help' | 'search' | 'dayNight' }
>

type Props = {
	/** Text is selected or an annotation's toolbar is open */
	hasTarget: boolean
	/** The toolbar is acting on an existing annotation */
	targetIsAnnotation: boolean
	onAnnotationAction: (action: AnnotationShortcutAction) => void
}

/**
 * The EPUB reader's shortcuts: annotate the selection (h/u/s/i/n/c, Delete), toggle a
 * bookmark (b), switch day/night (d), search (Ctrl/Cmd+F, `/`), go to a location (g) and the
 * shortcuts in the settings (?). Key presses inside the book's frame are forwarded here by the navigator, so
 * they work wherever the focus is (and the browser's find bar or quick find doesn't open);
 * typing in an input or a dialog is never intercepted.
 */
export default function EpubReaderShortcuts({
	hasTarget,
	targetIsAnnotation,
	onAnnotationAction,
}: Props) {
	const { translate } = useTranslate()
	const { isDarkVariant } = useTheme()
	const { openSettings, openSearch, openGoTo } = useEpubReaderControls()
	const theme = useReaderStore((state) => state.settings.theme ?? 'auto')
	const setSettings = useReaderStore((state) => state.setSettings)
	const { createBookmark, deleteBookmark, currentIsBookmarked, canBookmarkCurrent } =
		useEpubBookmark()

	// Read the latest values from the listener without re-subscribing on every change
	const stateRef = useRef({
		hasTarget,
		targetIsAnnotation,
		onAnnotationAction,
		openSettings,
		openSearch,
		openGoTo,
		toggleDayNight: () => {},
		toggleBookmark: () => {},
	})
	useEffect(() => {
		stateRef.current = {
			hasTarget,
			targetIsAnnotation,
			onAnnotationAction,
			openSettings,
			openSearch,
			openGoTo,
			toggleDayNight: () => setSettings({ theme: nextDayNightTheme(theme, isDarkVariant) }),
			// Feedback, as the top bar (and its bookmark icon) may be hidden
			toggleBookmark: () => {
				if (currentIsBookmarked) {
					deleteBookmark()
					toast(translate('shared.readerAnnotations.bookmarkRemoved'))
				} else if (canBookmarkCurrent) {
					createBookmark()
					toast(translate('shared.readerAnnotations.bookmarkAdded'))
				}
			},
		}
	})

	useEffect(() => {
		const handleKeyDown = (event: KeyboardEvent) => {
			if (event.defaultPrevented || isEditableTarget(event.target)) return

			const state = stateRef.current
			// g: go to a location (the contents, with their go-to field focused), as in the
			// other readers
			if (
				!state.hasTarget &&
				!event.ctrlKey &&
				!event.metaKey &&
				!event.altKey &&
				typeof event.key === 'string' &&
				event.key.toLowerCase() === 'g'
			) {
				event.preventDefault()
				state.openGoTo()
				return
			}
			const action = resolveReaderShortcut(event, state)
			if (!action) return

			event.preventDefault()
			switch (action.type) {
				case 'bookmark':
					state.toggleBookmark()
					break
				case 'help':
					state.openSettings('shortcuts')
					break
				case 'search':
					state.openSearch()
					break
				case 'dayNight':
					state.toggleDayNight()
					break
				default:
					state.onAnnotationAction(action)
			}
		}

		window.addEventListener('keydown', handleKeyDown)
		return () => window.removeEventListener('keydown', handleKeyDown)
	}, [])

	return null
}
