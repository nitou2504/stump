import { cn, IconButton } from '@stump/components'
import { SearchIcon } from 'lucide-react'
import { lazy, Suspense, useEffect, useState } from 'react'

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
 * series, with quick results, "See all" and "Surprise me". From tablet width it looks like a
 * search box with its shortcut; on phones it is an icon. List pages keep their own search in
 * the toolbar, which filters only that list.
 */
export default function GlobalSearch({ className }: { className?: string }) {
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
			<button
				type="button"
				onClick={show}
				onPointerEnter={loadDialog}
				onFocus={loadDialog}
				aria-label="Search the library"
				aria-keyshortcuts={isApple ? 'Meta+K' : 'Control+K'}
				className={cn(
					'h-8 w-48 xl:w-64 gap-2 pr-1.5 pl-2.5 text-sm md:flex hidden items-center rounded-md border border-border text-muted-foreground transition-colors',
					'hover:border-ring/50 hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none',
				)}
			>
				<SearchIcon className="h-4 w-4 shrink-0" aria-hidden />
				<span className="min-w-0 flex-1 truncate text-left">Search books, authors…</span>
				<kbd className="px-1.5 font-medium rounded leading-5 shrink-0 border border-border bg-muted font-sans text-[11px]">
					{GLOBAL_SEARCH_SHORTCUT}
				</kbd>
			</button>
			<IconButton
				variant="ghost"
				size="sm"
				className="md:hidden"
				onClick={show}
				aria-label="Search the library"
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
