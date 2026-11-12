import { useGraphQLMutation, usePrefetchFiles } from '@stump/client'
import { formatBytesSeparate } from '@stump/client'
import { DropdownItemGroup } from '@stump/components/dropdown/DropdownMenu'
import { extractErrorMessage, graphql, UserPermission } from '@stump/graphql'
import { useQueryClient } from '@tanstack/react-query'
import {
	Book,
	BookCheck,
	BookOpenCheck,
	FileSearch,
	HardDrive,
	Image,
	ImagePlus,
	ImageUp,
} from 'lucide-react'
import { useState } from 'react'
import { useLocation } from 'react-router'
import { toast } from 'sonner'

import { EntityHeader } from '@/components/sharedLayout'
import { useAppContext } from '@/context'
import { useStatLabels } from '@/hooks/useStatLabels'
import { useTranslate } from '@/hooks/useTranslate'
import { usePaths } from '@/paths'

import { CompleteSeriesConfirmation } from './CompleteSeriesConfirmation'
import { useSeriesContext } from './context'
import { usePrefetchSeriesBooks } from './queries'
import { SeriesOverviewSheet } from './SeriesOverviewSheet'
import { useSeriesThumbnailActions } from './useSeriesThumbnailActions'

const completeSeriesMutation = graphql(`
	mutation SeriesActionComplete($id: ID!) {
		finishSeriesProgress(id: $id)
	}
`)

const analyzeMutation = graphql(`
	mutation SeriesHeaderAnalyze($id: ID!) {
		analyzeSeries(id: $id)
	}
`)

export function SeriesHeader() {
	const { checkPermission, isServerOwner } = useAppContext()
	const {
		series: {
			id,
			resolvedName,
			path,
			stats,
			library: { id: libraryId, name: libraryName },
		},
	} = useSeriesContext()
	const { t, translate } = useTranslate()
	const labels = useStatLabels()

	const location = useLocation()

	const paths = usePaths()
	const formattedSize = stats.totalBytes ? formatBytesSeparate(stats.totalBytes) : null

	const [showCompleteSeriesConfirmation, setShowCompleteSeriesConfirmation] = useState(false)
	const [isOverviewSheetOpen, setIsOverviewSheetOpen] = useState(false)

	const client = useQueryClient()

	const onSuccess = () => {
		client.invalidateQueries({ queryKey: ['seriesBooks', id], exact: false })
	}

	const { mutate: completeSeries } = useGraphQLMutation(completeSeriesMutation, {
		onSuccess,
		onError: (error) => {
			console.error(error)
			toast.error(t('seriesHeader.errors.failedToUpdateCompletion'), {
				description: extractErrorMessage(error),
			})
		},
	})

	const thumbnail = useSeriesThumbnailActions(id)
	const { mutate: analyze, isPending: isAnalyzing } = useGraphQLMutation(analyzeMutation, {
		onSuccess: () => toast.success('Analysis started'),
		onError: (error) => {
			console.error(error)
			toast.error('Failed to start analysis')
		},
	})

	const canAnalyze = checkPermission(UserPermission.ManageLibrary)
	const canEditThumbnail = checkPermission(UserPermission.EditThumbnails)
	const canUpload = thumbnail.canUpload

	const actions = [
		{
			items: [
				{
					label: t('seriesHeader.actions.markAsRead'),
					leftIcon: <BookOpenCheck />,
					onClick: () => {
						setShowCompleteSeriesConfirmation(true)
					},
				},
			],
		},
		{
			items: [
				...(canAnalyze
					? [
							{
								label: translate('shared.bookActions.analyze'),
								leftIcon: <FileSearch />,
								disabled: isAnalyzing,
								onClick: () => analyze({ id }),
							},
						]
					: []),
				...(canEditThumbnail
					? [
							{
								label: translate('shared.common.thumbnail'),
								leftIcon: <Image />,
								subItems: [
									{
										label: t('thumbnailSelector.chooseFromBooks'),
										leftIcon: <ImagePlus />,
										onClick: thumbnail.openSelector,
									},
									...(canUpload
										? [
												{
													label: t('thumbnailSelector.uploadImage'),
													leftIcon: <ImageUp />,
													onClick: thumbnail.openUpload,
												},
											]
										: []),
								],
							},
						]
					: []),
			],
		},
	].filter((group) => group.items.length > 0) satisfies DropdownItemGroup[]

	const prefetchSeriesBooks = usePrefetchSeriesBooks()
	const prefetchFiles = usePrefetchFiles()

	const canAccessFiles = checkPermission(UserPermission.FileExplorer)

	const tabs = [
		{
			isActive: !!location.pathname.match(/\/series\/[^/]+\/books(\/.*)?$/),
			label: t('seriesHeader.tabs.books'),
			onHover: () => prefetchSeriesBooks(id),
			to: paths.seriesOverview(id),
		},
		...(canAccessFiles && path != null
			? [
					{
						isActive: !!location.pathname.match(/\/series\/[^/]+\/files(\/.*)?$/),
						label: translate('shared.common.files'),
						onHover: () =>
							prefetchFiles({
								path,
								fetchConfig: checkPermission(UserPermission.UploadFile),
							}),
						to: paths.seriesFileExplorer(id),
					},
				]
			: []),
	]

	// The books and how many are read, in words; the in-progress count and the reading time are in the
	// info sheet (the (i) button), labelled there
	const resolvedStats = stats
		? [
				{
					key: 'bookCount',
					icon: Book,
					value: labels.number(stats.bookCount),
					suffix: labels.word('books', stats.bookCount),
					title: labels.count('books', stats.bookCount),
				},
				{
					key: 'completedBooks',
					icon: BookCheck,
					value: labels.number(stats.completedBooks),
					suffix: labels.read(),
					title: translate('shared.common.infoSheetStats.completedBooks'),
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
			<CompleteSeriesConfirmation
				isOpen={showCompleteSeriesConfirmation}
				onCancel={() => setShowCompleteSeriesConfirmation(false)}
				onConfirm={() => {
					completeSeries({ id: id })
					setShowCompleteSeriesConfirmation(false)
				}}
			/>

			<EntityHeader
				name={resolvedName}
				parents={[
					{ label: translate('shared.common.libraries'), to: paths.libraries() },
					{ label: libraryName, to: paths.library(libraryId) },
				]}
				tabs={tabs}
				actions={actions}
				stats={resolvedStats}
				// the series settings page is the owner's (its route sends everybody else away)
				settingsLink={isServerOwner ? paths.seriesSettings(id) : undefined}
				onInfoClick={() => setIsOverviewSheetOpen(true)}
			/>

			{canEditThumbnail && thumbnail.dialogs}

			<SeriesOverviewSheet
				isOpen={isOverviewSheetOpen}
				onClose={() => setIsOverviewSheetOpen(false)}
			/>
		</>
	)
}
