import { ConfirmationModal } from '@stump/components'
import { AccessRole, EntityVisibility } from '@stump/graphql'
import { Book, GlobeLock, Layers, Library, Trash2 } from 'lucide-react'
import { useState } from 'react'
import { useLocation, useNavigate } from 'react-router'
import { toast } from 'sonner'

import { EntityHeader } from '@/components/sharedLayout'
import { useAppContext } from '@/context'
import { useStatLabels } from '@/hooks/useStatLabels'
import { useTranslate } from '@/hooks/useTranslate'
import { usePaths } from '@/paths'

import { useSmartListContext } from './context'
import { usePrefetchSmartList, useUnpublishSmartList } from './graphql'
import DeleteListConfirmation from './settings/danger/DeleteListConfirmation'

const LOCALE_BASE_KEY = 'userSmartListScene.navigation'
const withLocaleKey = (key: string) => `${LOCALE_BASE_KEY}.${key}`

const errorMessage = (error: unknown) =>
	error instanceof Error && error.message ? error.message : undefined

export default function UserSmartListHeader() {
	const { t, translate } = useTranslate()
	const labels = useStatLabels()
	const location = useLocation()
	const navigate = useNavigate()
	const paths = usePaths()
	const { user } = useAppContext()
	const { list, meta, viewerRole } = useSmartListContext()
	const { id, name } = list
	const { prefetch } = usePrefetchSmartList()
	const { unpublish, isPending: isUnpublishing } = useUnpublishSmartList(list)

	const [isConfirmingUnpublish, setIsConfirmingUnpublish] = useState(false)
	const [isConfirmingDelete, setIsConfirmingDelete] = useState(false)

	// The server owner doesn't change someone else's list: only take it back from the public, or
	// delete it. Everyone else who isn't its maker only reads it
	const isModerator =
		viewerRole === AccessRole.Reader &&
		user.isServerOwner &&
		list.visibility === EntityVisibility.Public

	const confirmUnpublish = async () => {
		try {
			await unpublish()
			toast.success(t('userSmartListScene.moderation.unpublishDone', { name }))
			setIsConfirmingUnpublish(false)
			// it is private now, and not the server owner's to see
			navigate(paths.smartLists(), { replace: true })
		} catch (error) {
			toast.error(t('userSmartListScene.moderation.unpublishError'), {
				description: errorMessage(error),
			})
		}
	}

	const tabs = [
		{
			isActive: !!location.pathname.match(/\/smart-lists\/[^/]+(\/items)?$/),
			label: t(withLocaleKey('items')),
			onHover: () => prefetch({ id }),
			to: paths.smartListItems(id),
		},
	]

	const resolvedStats = meta
		? [
				{
					key: 'bookCount',
					icon: Book,
					value: labels.number(meta.matchedBooks),
					suffix: labels.word('books', meta.matchedBooks),
				},
				{
					key: 'seriesCount',
					icon: Layers,
					value: labels.number(meta.matchedSeries),
					suffix: labels.word('series', meta.matchedSeries),
				},
				...(meta.matchedLibraries > 1
					? [
							{
								key: 'libraryCount',
								icon: Library,
								value: labels.number(meta.matchedLibraries),
								suffix: labels.word('libraries', meta.matchedLibraries),
							},
						]
					: []),
			]
		: undefined

	const moderatorActions = isModerator
		? [
				{
					items: [
						{
							label: t('userSmartListScene.moderation.makePrivate'),
							leftIcon: <GlobeLock />,
							onClick: () => setIsConfirmingUnpublish(true),
						},
						{
							label: t('userSmartListScene.moderation.delete'),
							leftIcon: <Trash2 />,
							variant: 'destructive' as const,
							onClick: () => setIsConfirmingDelete(true),
						},
					],
				},
			]
		: undefined

	return (
		<>
			<EntityHeader
				name={name}
				parents={[{ label: translate('shared.common.smartLists'), to: paths.smartLists() }]}
				tabs={tabs}
				stats={resolvedStats}
				actions={moderatorActions}
				// a list's settings are for its maker alone
				settingsLink={viewerRole === AccessRole.Reader ? undefined : paths.smartListSettings(id)}
			/>

			{isModerator && (
				<>
					<ConfirmationModal
						title={t('userSmartListScene.moderation.unpublishTitle')}
						description={t('userSmartListScene.moderation.unpublishDescription', {
							name,
							creator: list.creator?.username ?? '',
						})}
						isOpen={isConfirmingUnpublish}
						onClose={() => setIsConfirmingUnpublish(false)}
						onConfirm={confirmUnpublish}
						confirmIsLoading={isUnpublishing}
						confirmText={t('userSmartListScene.moderation.unpublishConfirm')}
					/>
					<DeleteListConfirmation
						id={id}
						isOpen={isConfirmingDelete}
						onClose={() => setIsConfirmingDelete(false)}
					/>
				</>
			)}
		</>
	)
}
