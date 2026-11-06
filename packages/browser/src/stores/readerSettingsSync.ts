import { useSDK } from '@stump/client'
import { graphql } from '@stump/graphql'
import { useEffect } from 'react'

import { usePdfReaderStore } from '@/components/readers/pdf/store'

import { useReaderStore } from './reader'

/**
 * Reading settings that follow the user: the reader theme and the other reader-wide
 * settings, the defaults for every EPUB, PDF or comic ("Make default") and each book's own
 * settings are saved on the server, per user, so every address, browser and device reads
 * the same ones (the browser keeps them per address, so `http://<ip>:10801` and the tailnet
 * address used to have their own).
 *
 * After sign-in the server's copy replaces the browser's (the first browser to sync seeds the
 * server with its own). A change is sent a second after the last edit, and when the tab is
 * hidden; the server's copy is read again when a tab comes back to the front, so tabs and
 * devices catch up with each other. Caches (page sizes, margins, reading timers) stay local.
 */

const query = graphql(`
	query MyReaderSettings {
		myReaderSettings
	}
`)

const mutation = graphql(`
	mutation UpdateMyReaderSettings($settings: String!) {
		updateMyReaderSettings(settings: $settings)
	}
`)

/** Bumped when the synced shape changes */
export const READER_SETTINGS_VERSION = 1
/** Wait this long after the last change before sending */
const SEND_DELAY_MS = 1000

type ReaderState = ReturnType<typeof useReaderStore.getState>
type PdfState = ReturnType<typeof usePdfReaderStore.getState>

export type ReaderSettingsSnapshot = {
	v: number
	reader: {
		settings: Omit<ReaderState['settings'], 'showToolBar'>
		formatDefaults: ReaderState['formatDefaults']
		bookPreferences: ReaderState['bookPreferences']
	}
	pdf: {
		books: PdfState['books']
		defaults: PdfState['defaults']
	}
}

/** What is synced of the two stores (not the toolbar's visibility, a moment's state) */
export function snapshotOf(reader: ReaderState, pdf: PdfState): ReaderSettingsSnapshot {
	// eslint-disable-next-line @typescript-eslint/no-unused-vars
	const { showToolBar, ...settings } = reader.settings
	return {
		v: READER_SETTINGS_VERSION,
		reader: {
			settings,
			formatDefaults: reader.formatDefaults ?? {},
			bookPreferences: reader.bookPreferences ?? {},
		},
		pdf: { books: pdf.books ?? {}, defaults: pdf.defaults ?? {} },
	}
}

/** A saved snapshot, or `null` when it is missing, unreadable or of another version */
export function parseSnapshot(text: string | null | undefined): ReaderSettingsSnapshot | null {
	if (!text) return null
	try {
		const value = JSON.parse(text) as Partial<ReaderSettingsSnapshot>
		if (value?.v !== READER_SETTINGS_VERSION || !value.reader || !value.pdf) return null
		return value as ReaderSettingsSnapshot
	} catch {
		return null
	}
}

/** Puts a snapshot into the stores (keeping the browser's own toolbar visibility) */
export function applySnapshot(snapshot: ReaderSettingsSnapshot) {
	useReaderStore.setState((state) => ({
		settings: { ...state.settings, ...snapshot.reader.settings },
		formatDefaults: snapshot.reader.formatDefaults ?? {},
		bookPreferences: snapshot.reader.bookPreferences ?? {},
	}))
	usePdfReaderStore.setState({
		books: snapshot.pdf.books ?? {},
		defaults: snapshot.pdf.defaults ?? {},
	})
}

const currentText = () =>
	JSON.stringify(snapshotOf(useReaderStore.getState(), usePdfReaderStore.getState()))

/** Keeps the reader settings in step with the server while a user is signed in */
export function useReaderSettingsSync(userId: string | undefined) {
	const { sdk } = useSDK()

	useEffect(() => {
		if (!userId) return undefined
		let cancelled = false
		/** Until the server's copy is in, local changes aren't sent (they'd overwrite it) */
		let ready = false
		let applying = false
		/** The text the server has, as far as this tab knows */
		let lastSynced: string | null = null
		let timer: number | undefined

		const send = async () => {
			window.clearTimeout(timer)
			timer = undefined
			const text = currentText()
			if (text === lastSynced) return
			try {
				await sdk.execute(mutation, { settings: text })
				lastSynced = text
			} catch (error) {
				console.warn('Failed to save the reading settings', error)
			}
		}

		const pull = async () => {
			try {
				const response = await sdk.execute(query)
				if (cancelled || timer !== undefined) return
				const text = response.myReaderSettings ?? null
				const snapshot = parseSnapshot(text)
				if (!snapshot) {
					// nothing saved yet (or an older shape): this browser's settings seed the server
					ready = true
					await send()
					return
				}
				if (text !== currentText()) {
					applying = true
					applySnapshot(snapshot)
					applying = false
				}
				lastSynced = currentText()
				ready = true
			} catch (error) {
				console.warn('Failed to load the reading settings', error)
			}
		}

		const onChange = () => {
			if (!ready || applying || cancelled) return
			window.clearTimeout(timer)
			timer = window.setTimeout(() => void send(), SEND_DELAY_MS)
		}
		const unsubscribeReader = useReaderStore.subscribe(onChange)
		const unsubscribePdf = usePdfReaderStore.subscribe(onChange)

		const onVisibility = () => {
			if (document.visibilityState === 'hidden') {
				if (timer !== undefined) void send()
			} else if (timer === undefined) {
				void pull()
			}
		}
		document.addEventListener('visibilitychange', onVisibility)

		void pull()

		return () => {
			cancelled = true
			if (timer !== undefined) void send()
			unsubscribeReader()
			unsubscribePdf()
			document.removeEventListener('visibilitychange', onVisibility)
		}
	}, [sdk, userId])
}
