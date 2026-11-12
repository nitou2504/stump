import { formatBytesSeparate, usePrefetchFiles } from '@stump/client'
import { UserPermission } from '@stump/graphql'
import { Book, BookPlus, HardDrive, Layers } from 'lucide-react'
import { useState } from 'react'
import { useLocation } from 'react-router'

import { DEFAULT_SERIES_ORDER_BY } from '@/components/filters/useFilterScene'
import { EntityHeader, HeaderIconButton } from '@/components/sharedLayout'
import { useAppContext } from '@/context'
import { useStatLabels } from '@/hooks/useStatLabels'
import { useTranslate } from '@/hooks/useTranslate'
import { usePaths } from '@/paths'

import { useLibraryContext } from './context'
import { LibraryOverviewSheet } from './LibraryOverviewSheet'
import { usePrefetchLibraryBooks } from './tabs/books/queries'
import { usePrefetchLibrarySeries } from './tabs/series/queries'

export default function LibraryHeader() {
	const location = useLocation()
	const { t, translate } = useTranslate()
	const labels = useStatLabels()
	const {
		library: { id, name, path, stats, config },
	} = useLibraryContext()
	const { checkPermission } = useAppContext()

	const [isOverviewSheetOpen, setIsOverviewSheetOpen] = useState(false)

	const paths = usePaths()

	const prefetchSeries = usePrefetchLibrarySeries()
	const prefetchBooks = usePrefetchLibraryBooks()
	const prefetchFiles = usePrefetchFiles()

	const handlePrefetchFiles = () => {
		prefetchFiles({ path, fetchConfig: checkPermission(UserPermission.UploadFile) })
	}

	const canAccessFiles = checkPermission(UserPermission.FileExplorer)
	const canAddBooks = checkPermission(UserPermission.AddBooks)
	const canManageLibrary = checkPermission(UserPermission.ManageLibrary)
	const hideSeriesView = config?.hideSeriesView ?? false
	const showOneshots = !!config?.oneshotsDirectory

	const formattedSize = stats?.totalBytes ? formatBytesSeparate(stats.totalBytes) : null

	const tabs = [
		...(!hideSeriesView
			? [
					{
						isActive: !!location.pathname.match(/\/libraries\/[^/]+\/?(series)?$/),
						label: t('libraryHeader.tabs.series'),
						onHover: () => prefetchSeries(id),
						to: paths.librarySeries(id),
					},
				]
			: []),
		...(showOneshots
			? [
					{
						isActive: !!location.pathname.match(/\/libraries\/[^/]+\/oneshots(\/.*)?$/),
						label: t('libraryHeader.tabs.oneshots'),
						onHover: () =>
							prefetchSeries(id, {
								filter: [{ isOneshot: true }],
								orderBy: DEFAULT_SERIES_ORDER_BY,
							}),
						to: paths.libraryOneshots(id),
					},
				]
			: []),
		{
			isActive: !!location.pathname.match(/\/libraries\/[^/]+\/books(\/.*)?$/),
			label: t('libraryHeader.tabs.books'),
			onHover: () => prefetchBooks(id),
			to: paths.libraryBooks(id),
		},
		...(canAccessFiles
			? [
					{
						isActive: !!location.pathname.match(/\/libraries\/[^/]+\/files(\/.*)?$/),
						label: translate('shared.common.files'),
						onHover: () => handlePrefetchFiles(),
						to: paths.libraryFileExplorer(id),
					},
				]
			: []),
	]

	// What the library is, in words: the numbers about the reader's own progress and time are in the
	// info sheet (the (i) button), labelled there
	const resolvedStats = stats
		? [
				...(!hideSeriesView
					? [
							{
								key: 'seriesCount',
								icon: Layers,
								value: labels.number(stats.seriesCount),
								suffix: labels.word('series', stats.seriesCount),
								title: labels.count('series', stats.seriesCount),
							},
						]
					: []),
				{
					key: 'bookCount',
					icon: Book,
					value: labels.number(stats.bookCount),
					suffix: labels.word('books', stats.bookCount),
					title: labels.count('books', stats.bookCount),
				},
				...(formattedSize
					? [
							{
								key: 'totalBytes',
								icon: HardDrive,
								value: formattedSize.value,
								suffix: formattedSize.unit,
								title: translate('shared.common.infoSheetStats.size'),
							},
						]
					: []),
			]
		: undefined

	return (
		<>
			<EntityHeader
				name={name}
				parents={[{ label: translate('shared.common.libraries'), to: paths.libraries() }]}
				tabs={tabs}
				stats={resolvedStats}
				settingsLink={canManageLibrary ? paths.libraryManage(id) : undefined}
				onInfoClick={() => setIsOverviewSheetOpen(true)}
				endActions={
					canAddBooks ? (
						<HeaderIconButton
							label={t('sidebar.buttons.addBooks')}
							icon={BookPlus}
							to={paths.addBooks(id)}
						/>
					) : undefined
				}
			/>

			<LibraryOverviewSheet
				isOpen={isOverviewSheetOpen}
				onClose={() => setIsOverviewSheetOpen(false)}
			/>
		</>
	)
}
