import { cn, IconButton } from '@stump/components'
import { SearchIcon } from 'lucide-react'
import { lazy, Suspense, useEffect, useState } from 'react'

import { useTranslate } from '@/hooks/useTranslate'

import { GLOBAL_SEARCH_SHORTCUT, isApple } from './shortcut'

// the dialog (and cmdk) load on first use or on hover, not with every page
const loadDialog = () => import('./GlobalSearchDialog')
const GlobalSearchDialog = lazy(loadDialog)

/** Ctrl/Cmd+K */
function isOpenShortcut(event: KeyboardEvent) {
	return (
		(event.ctrlKey || event.metaKey) &&
		!event.altKey &&
		!event.shiftKey &&
		event.key.toLowerCase() === 'k'
	)
}

/**
 * The mounted searches, latest last: when a page shows two title bars (e.g. settings inside a
 * layout), only the latest answers the shortcut, so one press doesn't open and close it again
 */
const mounted: symbol[] = []

/**
 * The title bar's search, on every page with a title bar: the whole server's books and
 * series, with quick results, "See all" and "Surprise me". It is an icon at every width (the
 * box it used to be only opened this dialog), and its tooltip names the shortcut. List pages
 * keep their own search in the toolbar, which filters only that list.
 */
export default function GlobalSearch({ className }: { className?: string }) {
	const { t } = useTranslate()
	const [open, setOpen] = useState(false)
	const [hasOpened, setHasOpened] = useState(false)

	const [id] = useState(() => Symbol('global-search'))
	useEffect(() => {
		mounted.push(id)
		return () => {
			mounted.splice(mounted.indexOf(id), 1)
		}
	}, [id])

	useEffect(() => {
		const onKeyDown = (event: KeyboardEvent) => {
			if (!isOpenShortcut(event) || event.defaultPrevented) return
			if (mounted[mounted.length - 1] !== id) return
			// a dialog of the page (not this one) keeps its keys
			if (!open && document.querySelector('[role="dialog"][data-state="open"]')) return
			event.preventDefault()
			setHasOpened(true)
			setOpen((current) => !current)
		}
		window.addEventListener('keydown', onKeyDown)
		return () => window.removeEventListener('keydown', onKeyDown)
	}, [open, id])

	const show = () => {
		setHasOpened(true)
		setOpen(true)
	}

	return (
		<div className={cn('flex shrink-0 items-center', className)}>
			<IconButton
				variant="ghost"
				size="sm"
				onClick={show}
				onPointerEnter={loadDialog}
				onFocus={loadDialog}
				aria-label={t('globalSearch.label')}
				aria-keyshortcuts={isApple ? 'Meta+K' : 'Control+K'}
				title={`${t('globalSearch.title')} · ${GLOBAL_SEARCH_SHORTCUT}`}
			>
				<SearchIcon className="h-4 w-4" />
			</IconButton>

			{hasOpened && (
				<Suspense fallback={null}>
					<GlobalSearchDialog open={open} onOpenChange={setOpen} />
				</Suspense>
			)}
		</div>
	)
}
