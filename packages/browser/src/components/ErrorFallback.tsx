import { Button, ButtonOrLink, useBodyLock } from '@stump/components'
import { ExternalLink } from 'lucide-react'
import { ErrorInfo } from 'react'
import { FallbackProps } from 'react-error-boundary'
import { toast } from 'sonner'

import { useTranslate } from '@/hooks/useTranslate'

import { copyTextToClipboard } from '../utils/misc'
import { Owl } from './Owl'

/**
 * Details of the last error caught by the app's error boundary. React only hands the
 * component stack to the boundary's `onError`, so it is kept here for the copy button.
 */
const lastError: { componentStack?: string | null; url?: string } = {}

/** Pass as the error boundary's `onError` to capture the details copied by the fallback */
export function recordBoundaryError(error: Error, info: ErrorInfo) {
	lastError.componentStack = info.componentStack
	lastError.url = window.location.href
	console.error('Error boundary caught:', error, info.componentStack)
}

/**
 * The text copied by the fallback's copy button: enough to locate the failing component
 * in a production build (map the chunk positions with the build's hidden source maps)
 */
function errorReport(error: Error) {
	return [
		`${error.name}: ${error.message}`,
		`URL: ${lastError.url ?? window.location.href}`,
		`Previous page: ${document.referrer || 'n/a'}`,
		`User agent: ${navigator.userAgent}`,
		'',
		'Stack:',
		error.stack ?? '(none)',
		'',
		'Component stack:',
		lastError.componentStack ?? '(none)',
	].join('\n')
}

// TODO: take in platform?
export function ErrorFallback({ error, resetErrorBoundary }: FallbackProps) {
	useBodyLock()
	const { t } = useTranslate()

	function copyErrorStack() {
		copyTextToClipboard(errorReport(error)).then(() => {
			toast.success(t('errorScene.copiedDetails'))
		})
	}

	return (
		<div
			data-tauri-drag-region
			data-testid="error-fallback"
			className="flex h-full w-full flex-col items-center justify-center overflow-hidden"
		>
			<Owl owl="error" className="mx-auto" />

			<div className="max-w-sm sm:max-w-md md:max-w-xl">
				<div className="text-left">
					<h1 className="text-4xl font-semibold text-foreground" data-testid="criticalErrorHeading">
						{t('errorScene.criticalHeading')}
					</h1>
					<p className="mt-1.5 text-lg text-foreground">
						{error.message || t('errorScene.emptyMessage')}
					</p>
				</div>
				<div className="gap-3 pt-3 flex w-full items-center">
					<ButtonOrLink
						onClick={resetErrorBoundary}
						title={t('errorScene.buttons.goHomeTitle')}
						forceAnchor
						href="/"
					>
						{t('errorScene.buttons.goHome')}
					</ButtonOrLink>
					<ButtonOrLink
						title={t('errorScene.buttons.reportTitle')}
						href="https://github.com/stumpapp/stump/issues/new/choose"
						target="_blank"
					>
						{t('errorScene.buttons.report')} <ExternalLink className="ml-2 h-4 w-4" />
					</ButtonOrLink>
					{(error.stack || lastError.componentStack) && (
						<Button
							title={t('errorScene.buttons.copyTitle')}
							onClick={copyErrorStack}
							variant="ghost"
						>
							{t('errorScene.buttons.copy')}
						</Button>
					)}
				</div>
			</div>
		</div>
	)
}
