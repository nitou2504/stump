import { useGraphQL } from '@stump/client'
import { Button, NativeSelect } from '@stump/components'
import { CheckCheck, Inbox } from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'
import { Helmet } from 'react-helmet'
import { useSearchParams } from 'react-router'

import { SceneContainer } from '@/components/container'
import Pagination from '@/components/Pagination'
import AcceptAllSheet from '@/scenes/intake/AcceptAllSheet'
import { useIntakeActions, usePendingIntake } from '@/scenes/intake/hooks'
import { intakeLibrariesQuery } from '@/scenes/intake/queries'
import { approvableInBulk } from '@/scenes/intake/reviewState'

import ReviewCard from './ReviewCard'
import UserLimits from './UserLimits'

export const APPROVALS_PAGE_SIZE = 20

/**
 * Settings → Book approvals: the books other people sent, oldest first, for the server owner and
 * those who manage libraries. Each can be changed (where it goes, its tags, its age rating),
 * approved (it is filed into the library) or sent back with a note.
 */
export default function BookApprovalsScene() {
	const [params, setParams] = useSearchParams()
	const libraryId = params.get('library') || undefined
	const page = Math.max(1, Number.parseInt(params.get('page') ?? '', 10) || 1)

	const { data: librariesData } = useGraphQL(intakeLibrariesQuery, ['intakeLibraries'])
	const libraries = useMemo(() => librariesData?.intakeLibraries ?? [], [librariesData])
	const { items, totalCount, isLoading } = usePendingIntake({
		libraryId,
		page,
		pageSize: APPROVALS_PAGE_SIZE,
	})
	const actions = useIntakeActions()
	const [approvingAll, setApprovingAll] = useState(false)

	const pages = Math.max(1, Math.ceil(totalCount / APPROVALS_PAGE_SIZE))
	const setParam = (key: string, value?: string) =>
		setParams(
			(prev) => {
				if (value) prev.set(key, value)
				else prev.delete(key)
				return prev
			},
			{ replace: true },
		)

	// the last book of the last page was approved: the page is gone, go back to the new last one
	useEffect(() => {
		if (isLoading || page <= pages) return
		setParams(
			(prev) => {
				if (pages > 1) prev.set('page', String(pages))
				else prev.delete('page')
				return prev
			},
			{ replace: true },
		)
	}, [isLoading, page, pages, setParams])

	// a book with something left to fix can't be approved, and one that may already be in the
	// library (or failed before) is for its own card: they are left out of "Approve all"
	const approvable = useMemo(() => items.filter(approvableInBulk), [items])
	const skipped = items.length - approvable.length

	return (
		<SceneContainer variant="settings">
			<Helmet>
				<title>Stump | Book approvals</title>
			</Helmet>

			<section aria-label="Books waiting for approval" className="gap-4 flex flex-col">
				<div className="gap-3 flex flex-wrap items-end justify-between">
					<p className="text-sm text-muted-foreground">
						{totalCount > 0
							? `${totalCount} ${totalCount === 1 ? 'book is' : 'books are'} waiting. They join the library when you approve them.`
							: 'Books other people add wait here until you approve them.'}
					</p>
					<div className="gap-2 flex flex-wrap items-center">
						{libraries.length > 1 && (
							<NativeSelect
								size="sm"
								className="text-base md:text-sm w-auto"
								aria-label="Filter by library"
								value={libraryId ?? ''}
								options={[
									{ label: 'All libraries', value: '' },
									...libraries.map((library) => ({ label: library.name, value: library.id })),
								]}
								onChange={(e) => {
									setParams(
										(prev) => {
											if (e.target.value) prev.set('library', e.target.value)
											else prev.delete('library')
											prev.delete('page')
											return prev
										},
										{ replace: true },
									)
								}}
							/>
						)}
						{approvable.length > 1 && (
							<Button
								size="sm"
								variant="secondary"
								onClick={() => setApprovingAll(true)}
								disabled={actions.isApproving || actions.isReviewing}
							>
								<CheckCheck /> Approve all ({approvable.length})
							</Button>
						)}
					</div>
				</div>

				{items.length > 0 ? (
					<div className="gap-4 flex flex-col">
						{items.map((item) => (
							<ReviewCard key={item.id} item={item} libraries={libraries} actions={actions} />
						))}
					</div>
				) : (
					!isLoading && (
						<div className="gap-2 py-10 flex flex-col items-center text-center text-muted-foreground">
							<Inbox className="h-8 w-8" aria-hidden />
							<p className="text-sm">Nothing is waiting for approval.</p>
						</div>
					)
				)}

				{pages > 1 && (
					<Pagination
						position="bottom"
						pages={pages}
						currentPage={Math.min(page, pages)}
						onChangePage={(next) => setParam('page', next > 1 ? String(next) : undefined)}
					/>
				)}
			</section>

			<UserLimits />

			<AcceptAllSheet
				items={approvable}
				open={approvingAll}
				verb="approve"
				description={`Untick any genre that doesn’t fit. Each book is added to the library the way the page shows it.${
					skipped > 0
						? ` ${skipped} ${skipped === 1 ? 'book is' : 'books are'} left out: ${skipped === 1 ? 'it has' : 'they have'} something to fix, may already be in the library, or failed. ${skipped === 1 ? 'It stays' : 'They stay'} here for you to open.`
						: ''
				}`}
				isPending={actions.isApproving || actions.isReviewing}
				onClose={() => setApprovingAll(false)}
				onConfirm={(entries) => {
					actions.approveWithGenres(entries)
					setApprovingAll(false)
				}}
			/>
		</SceneContainer>
	)
}
