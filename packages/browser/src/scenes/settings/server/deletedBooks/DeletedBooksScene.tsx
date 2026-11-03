import { useGraphQL } from '@stump/client'
import { Button, CheckBox, cn, NativeSelect } from '@stump/components'
import { UserPermission } from '@stump/graphql'
import { RotateCcw, Trash2, X } from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'
import { Helmet } from 'react-helmet'
import { useSearchParams } from 'react-router'

import { SceneContainer } from '@/components/container'
import { useAppContext } from '@/context'

import { EmptyConfirmation, PurgeConfirmation } from './Confirmations'
import { books } from './countdown'
import DeletedBookRow, { type DeletedBookEntry } from './DeletedBookRow'
import PurgedBooks from './PurgedBooks'
import { deletedBooksLibrariesQuery, deletedBooksQuery, deletedBooksSettingsQuery } from './queries'
import RetentionPolicy from './RetentionPolicy'
import { useDeletedBooksActions } from './useDeletedBooksActions'

/** The current time, refreshed every minute so countdowns stay right on an open page */
function useNow() {
	const [now, setNow] = useState(() => new Date())
	useEffect(() => {
		const timer = window.setInterval(() => setNow(new Date()), 60_000)
		return () => window.clearInterval(timer)
	}, [])
	return now
}

/**
 * Settings → Deleted books: the books removed from their libraries, waiting to be restored or
 * deleted permanently (by hand, or by the daily purge once the retention is over)
 */
export default function DeletedBooksScene() {
	const { checkPermission } = useAppContext()
	const canEditRetention = checkPermission(UserPermission.ManageServer)
	const now = useNow()

	const [params, setParams] = useSearchParams()
	const libraryFilter = params.get('library') ?? ''
	const setLibraryFilter = (id: string) =>
		setParams(
			(prev) => {
				if (id) prev.set('library', id)
				else prev.delete('library')
				return prev
			},
			{ replace: true },
		)

	const { data, isLoading } = useGraphQL(deletedBooksQuery, ['deletedBooks'], {})
	const { data: settingsData } = useGraphQL(deletedBooksSettingsQuery, ['deletedBooksSettings'])
	const { data: librariesData } = useGraphQL(deletedBooksLibrariesQuery, ['deletedBooksLibraries'])
	const settings = settingsData?.deletedBooksSettings
	const actions = useDeletedBooksActions()

	const entries = useMemo(() => data?.deletedBooks ?? [], [data])
	const visible = useMemo(
		() => (libraryFilter ? entries.filter((e) => e.libraryId === libraryFilter) : entries),
		[entries, libraryFilter],
	)
	const libraryOptions = useMemo(() => {
		const counts = new Map<string, number>()
		for (const entry of entries) counts.set(entry.libraryId, (counts.get(entry.libraryId) ?? 0) + 1)
		const libraries = librariesData?.libraries.nodes ?? []
		return [
			{ label: `All libraries (${entries.length})`, value: '' },
			...libraries
				.filter((library) => counts.has(library.id) || library.id === libraryFilter)
				.map((library) => ({
					label: `${library.name} (${counts.get(library.id) ?? 0})`,
					value: library.id,
				})),
		]
	}, [entries, librariesData, libraryFilter])
	const filteredLibraryName = libraryFilter
		? librariesData?.libraries.nodes.find((library) => library.id === libraryFilter)?.name
		: undefined

	// the selection only ever holds books on screen
	const [selected, setSelected] = useState<Set<string>>(() => new Set())
	const selectedEntries = visible.filter((entry) => selected.has(entry.book.id))
	useEffect(() => {
		const onScreen = new Set(visible.map((entry) => entry.book.id))
		setSelected((prev) => {
			const kept = [...prev].filter((id) => onScreen.has(id))
			return kept.length === prev.size ? prev : new Set(kept)
		})
	}, [visible])
	const allSelected = visible.length > 0 && selectedEntries.length === visible.length
	const toggle = (id: string, on: boolean) =>
		setSelected((prev) => {
			const next = new Set(prev)
			if (on) next.add(id)
			else next.delete(id)
			return next
		})
	const toggleAll = (on: boolean) =>
		setSelected(on ? new Set(visible.map((entry) => entry.book.id)) : new Set())

	const [purging, setPurging] = useState<DeletedBookEntry[] | null>(null)
	const [emptying, setEmptying] = useState(false)
	const busy = actions.isRestoring || actions.isPurging

	const restore = async (targets: DeletedBookEntry[]) => {
		const ok = await actions.restore(
			targets.map((entry) => entry.book.id),
			targets.length === 1 ? targets[0]?.book.resolvedName : undefined,
		)
		if (ok) toggleAll(false)
	}

	const confirmPurge = async () => {
		if (!purging) return
		const ok = await actions.purge(
			purging.map((entry) => entry.book.id),
			purging.length === 1 ? purging[0]?.book.resolvedName : undefined,
		)
		if (ok) {
			setPurging(null)
			toggleAll(false)
		}
	}

	const confirmEmpty = async () => {
		const ok = await actions.empty(libraryFilter || undefined)
		if (ok) {
			setEmptying(false)
			toggleAll(false)
		}
	}

	const retentionDays = settings?.retentionDays ?? 14
	const quarantine = {
		quarantineDir: settings?.quarantineDir,
		quarantineReady: settings?.quarantineReady ?? false,
		now,
	}

	return (
		<SceneContainer variant="settings">
			<Helmet>
				<title>Stump | Deleted books</title>
			</Helmet>

			{settings && (
				<RetentionPolicy
					settings={settings}
					canEditRetention={canEditRetention}
					isSaving={actions.isSavingRetention}
					onRetentionChange={actions.setRetention}
				/>
			)}

			<section aria-label="Books in Deleted books" className="gap-3 flex flex-col">
				{(visible.length > 0 || libraryFilter) && (
					// stuck under the page header, on an opaque strip so rows don't show at its corners
					<div className="top-12 pb-1 -mb-1 sticky z-10 bg-background">
						<div
							className={cn(
								'gap-x-3 gap-y-2 px-3 py-2 min-h-12 flex flex-wrap items-center rounded-lg border',
								selectedEntries.length > 0
									? 'shadow-sm border-primary/40 bg-background'
									: 'border-transparent bg-background',
							)}
							data-testid="deleted-books-toolbar"
						>
							<CheckBox
								checked={allSelected ? true : selectedEntries.length > 0 ? 'indeterminate' : false}
								onCheckedChange={(checked) => toggleAll(checked === true)}
								disabled={visible.length === 0}
								aria-label={allSelected ? 'Select none' : `Select all ${visible.length}`}
							/>
							{selectedEntries.length > 0 ? (
								<>
									<span className="text-sm font-medium tabular-nums">
										{selectedEntries.length} of {visible.length} selected
									</span>
									<Button
										size="icon"
										variant="ghost"
										className="md:order-3 md:ml-0 order-1 ml-auto"
										aria-label="Clear the selection"
										onClick={() => toggleAll(false)}
									>
										<X aria-hidden />
									</Button>
									<div className="gap-2 md:order-1 md:ml-auto md:w-auto order-2 flex w-full">
										<Button
											size="sm"
											variant="outline"
											className="md:flex-none flex-1"
											disabled={busy}
											onClick={() => restore(selectedEntries)}
										>
											<RotateCcw aria-hidden />
											Restore {selectedEntries.length}
										</Button>
										<Button
											size="sm"
											variant="destructive"
											className="md:flex-none flex-1"
											disabled={busy}
											onClick={() => setPurging(selectedEntries)}
										>
											<Trash2 aria-hidden />
											Delete {selectedEntries.length} permanently
										</Button>
									</div>
								</>
							) : (
								<>
									<span className="text-sm text-muted-foreground tabular-nums">
										{isLoading ? 'Loading…' : books(visible.length)}
									</span>
									{libraryOptions.length > 2 || libraryFilter ? (
										<NativeSelect
											size="sm"
											className="text-base md:text-sm w-auto max-w-[14rem]"
											value={libraryFilter}
											options={libraryOptions}
											aria-label="Filter by library"
											onChange={(event) => setLibraryFilter(event.target.value)}
										/>
									) : null}
									<Button
										size="sm"
										variant="ghost"
										className="ml-auto text-destructive hover:text-destructive"
										disabled={visible.length === 0 || busy}
										aria-label={
											filteredLibraryName ? 'Delete all permanently' : 'Empty Deleted books'
										}
										onClick={() => setEmptying(true)}
									>
										<Trash2 aria-hidden />
										<span className="sm:hidden">
											{filteredLibraryName ? 'Delete all' : 'Empty'}
										</span>
										<span className="sm:inline hidden">
											{filteredLibraryName ? 'Delete all permanently' : 'Empty Deleted books'}
										</span>
									</Button>
								</>
							)}
						</div>
					</div>
				)}

				{!isLoading && visible.length === 0 ? (
					<div className="px-6 py-10 rounded-lg border border-dashed border-border text-center">
						<p className="font-medium">Nothing in Deleted books</p>
						<p className="mt-1 text-sm text-muted-foreground">
							{filteredLibraryName
								? `No book removed from ${filteredLibraryName} is waiting here.`
								: 'Books removed from their page (“Remove from library”) wait here until they are restored or deleted permanently.'}
						</p>
					</div>
				) : (
					<ul className="divide-y divide-border overflow-hidden rounded-lg border border-border">
						{visible.map((entry) => (
							<DeletedBookRow
								key={entry.book.id}
								entry={entry}
								selected={selected.has(entry.book.id)}
								onSelect={(on) => toggle(entry.book.id, on)}
								onRestore={() => restore([entry])}
								onPurge={() => setPurging([entry])}
								retentionDays={retentionDays}
								now={now}
								canMoveFiles={quarantine.quarantineReady}
								busy={busy}
							/>
						))}
					</ul>
				)}
			</section>

			<PurgedBooks onPutBack={actions.putBack} isPuttingBack={actions.isPuttingBack} />

			<PurgeConfirmation
				entries={purging}
				isPending={actions.isPurging}
				onConfirm={confirmPurge}
				onClose={() => setPurging(null)}
				{...quarantine}
			/>
			<EmptyConfirmation
				isOpen={emptying}
				entries={visible}
				libraryName={filteredLibraryName}
				isPending={actions.isPurging}
				onConfirm={confirmEmpty}
				onClose={() => setEmptying(false)}
				{...quarantine}
			/>
		</SceneContainer>
	)
}
