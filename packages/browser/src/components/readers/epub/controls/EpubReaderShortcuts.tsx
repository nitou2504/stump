import { useEffect, useRef } from 'react'
import { toast } from 'sonner'

import { useTranslate } from '@/hooks/useTranslate'

import {
	isEditableTarget,
	type ReaderShortcutAction,
	resolveReaderShortcut,
} from '../../annotations/shortcuts'
import { useEpubReaderControls } from '../context'
import { useEpubBookmark } from './useEpubBookmark'

type AnnotationShortcutAction = Exclude<ReaderShortcutAction, { type: 'bookmark' | 'help' }>

type Props = {
	/** Text is selected or an annotation's toolbar is open */
	hasTarget: boolean
	/** The toolbar is acting on an existing annotation */
	targetIsAnnotation: boolean
	onAnnotationAction: (action: AnnotationShortcutAction) => void
}

/**
 * The EPUB reader's letter shortcuts: annotate the selection (h/u/s/i/n, Delete), toggle a
 * bookmark (b) and show the shortcuts (?). Key presses inside the book's frame are
 * forwarded here by the navigator, so they work wherever the focus is; typing in an input
 * or a dialog is never intercepted.
 */
export default function EpubReaderShortcuts({
	hasTarget,
	targetIsAnnotation,
	onAnnotationAction,
}: Props) {
	const { translate } = useTranslate()
	const { shortcutsHelpOpen, setShortcutsHelpOpen } = useEpubReaderControls()
	const { createBookmark, deleteBookmark, currentIsBookmarked, canBookmarkCurrent } =
		useEpubBookmark()

	// Read the latest values from the listener without re-subscribing on every change
	const stateRef = useRef({
		hasTarget,
		targetIsAnnotation,
		onAnnotationAction,
		shortcutsHelpOpen,
		setShortcutsHelpOpen,
		toggleBookmark: () => {},
	})
	useEffect(() => {
		stateRef.current = {
			hasTarget,
			targetIsAnnotation,
			onAnnotationAction,
			shortcutsHelpOpen,
			setShortcutsHelpOpen,
			// Feedback, as the header (and its bookmark icon) may be hidden
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
			const action = resolveReaderShortcut(event, state)
			if (!action) return

			event.preventDefault()
			if (action.type === 'bookmark') {
				state.toggleBookmark()
			} else if (action.type === 'help') {
				state.setShortcutsHelpOpen(!state.shortcutsHelpOpen)
			} else {
				state.onAnnotationAction(action)
			}
		}

		window.addEventListener('keydown', handleKeyDown)
		return () => window.removeEventListener('keydown', handleKeyDown)
	}, [])

	return null
}
