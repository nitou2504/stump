import { useGraphQL } from '@stump/client'
import { Button, NativeSelect } from '@stump/components'
import { CheckCheck, Inbox } from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'
import { Helmet } from 'react-helmet'
import { useSearchParams } from 'react-router'

import { SceneContainer } from '@/components/container'
import Pagination from '@/components/Pagination'
import { useTranslate } from '@/hooks/useTranslate'
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
/** The words of the page are all under one key in the locale files */
const key = (name: string) => `settingsScene.server/bookApprovals.${name}`

export default function BookApprovalsScene() {
	const { t } = useTranslate()
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
				<title>Stump | {t(key('title'))}</title>
			</Helmet>

			<section aria-label={t(key('sectionLabel'))} className="gap-4 flex flex-col">
				<div className="gap-3 flex flex-wrap items-center justify-between">
					<p className="text-sm text-muted-foreground">
						{totalCount > 0 ? t(key('waiting'), { count: totalCount }) : t(key('emptyHelp'))}
					</p>
					<div className="gap-2 flex flex-wrap items-center">
						{libraries.length > 1 && (
							<NativeSelect
								size="sm"
								className="text-base md:text-sm w-auto"
								aria-label={t(key('filter'))}
								value={libraryId ?? ''}
								options={[
									{ label: t(key('allLibraries')), value: '' },
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
								<CheckCheck /> {t(key('approveAll'), { count: approvable.length })}
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
						<div className="gap-2 py-12 flex flex-col items-center text-center">
							<span className="size-12 grid place-items-center rounded-full bg-muted text-muted-foreground">
								<Inbox className="h-6 w-6" aria-hidden />
							</span>
							<p className="text-sm font-medium">{t(key('empty'))}</p>
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
				description={`${t(key('acceptAll.description'))}${
					skipped > 0 ? ` ${t(key('acceptAll.skipped'), { count: skipped })}` : ''
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
