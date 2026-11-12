import { captureJoinToken } from '@stump/browser'
import React from 'react'
import { createRoot } from 'react-dom/client'
import { registerSW } from 'virtual:pwa-register'

import App from './App'

function registerServiceWorkerWhenIdle() {
	if (!import.meta.env.PROD || !('serviceWorker' in navigator)) return

	const doRegister = () => registerSW()

	if (document.readyState === 'complete') {
		// eslint-disable-next-line @typescript-eslint/no-unused-expressions
		'requestIdleCallback' in globalThis
			? globalThis.requestIdleCallback(doRegister)
			: globalThis.setTimeout(doRegister, 0)
		return
	}

	globalThis.addEventListener(
		'load',
		() => {
			// eslint-disable-next-line @typescript-eslint/no-unused-expressions
			'requestIdleCallback' in globalThis
				? globalThis.requestIdleCallback(doRegister)
				: globalThis.setTimeout(doRegister, 0)
		},
		{ once: true },
	)
}

// an invite link's token (the address's fragment on /join) leaves the address bar before anything
// renders: an error report copies the address, and the browser's history would keep it
captureJoinToken()

const rootElement = document.getElementById('root')

if (!rootElement) {
	throw new Error('Root element not found')
}

const root = createRoot(rootElement)
root.render(
	<React.StrictMode>
		<App />
	</React.StrictMode>,
)

registerServiceWorkerWhenIdle()
