import { useDirectoryListing, UseDirectoryListingFile, useSDK } from '@stump/client'
import { InterfaceLayout } from '@stump/graphql'
import { useCallback } from 'react'
import { useNavigate } from 'react-router'
import { toast } from 'sonner'

import { useLayoutMode } from '@/hooks/useLayoutMode'
import { usePaths } from '@/paths'

import { ExplorerContext, ExplorerLayout, IExplorerContext } from './context'
import FileExplorer from './FileExplorer'
import FileExplorerFooter, { FOOTER_HEIGHT } from './FileExplorerFooter'
import FileExplorerHeader from './FileExplorerHeader'
import { getBook } from './FileThumbnail'

type Props = Pick<IExplorerContext, 'libraryID' | 'rootPath' | 'uploadConfig'>

// TODO: refactor to match other explore scenes, e.g. sticky header + fixed footer + window scrolling

export default function FileExplorerProvider({ rootPath, ...ctx }: Props) {
	const paths = usePaths()
	const navigate = useNavigate()
	const { sdk } = useSDK()

	// Grid or table follows the global layout preference, like every other list
	const { layoutMode, setLayoutMode } = useLayoutMode()
	const layout: ExplorerLayout = layoutMode === InterfaceLayout.Table ? 'table' : 'grid'

	// TODO: I need to store location.state somewhere so that when the user uses native navigation,
	// their history, or at the very least where they left off, is persisted.
	const {
		entries,
		setPath,
		path,
		goForward,
		goBack,
		canGoBack,
		canGoForward,
		refetch,
		canLoadMore,
		loadMore,
	} = useDirectoryListing({
		enforcedRoot: rootPath,
		initialPath: rootPath,
	})

	const handleSelect = async (entry: UseDirectoryListingFile) => {
		if (entry.isDirectory) {
			setPath(entry.path)
		} else {
			try {
				const entity = await getBook(entry.path, sdk)
				if (entity) {
					navigate(paths.bookOverview(entity.id), {
						state: {
							forward_path: path,
						},
					})
				} else {
					toast.error('No associated DB entry found for this file')
				}
			} catch (err) {
				console.error(err)
				toast.error('An unknown error occurred')
			}
		}
	}

	const changeLayout = (newLayout: ExplorerLayout) =>
		setLayoutMode(newLayout === 'table' ? InterfaceLayout.Table : InterfaceLayout.Grid)

	const onLoadMore = useCallback(() => {
		if (canLoadMore) {
			loadMore()
		}
	}, [canLoadMore, loadMore])

	return (
		<ExplorerContext.Provider
			value={{
				canGoBack: canGoBack && path !== rootPath,
				canGoForward,
				currentPath: path,
				files: entries,
				goBack,
				goForward,
				layout,
				navigateToPath: setPath,
				onSelect: handleSelect,
				refetch,
				rootPath,
				setLayout: changeLayout,
				canLoadMore,
				loadMore: onLoadMore,
				...ctx,
			}}
		>
			<div className="min-h-0 flex flex-1 flex-col">
				<FileExplorerHeader />
				<div
					className="flex-1"
					style={{
						marginBottom: FOOTER_HEIGHT,
					}}
				>
					<FileExplorer />
				</div>
				<FileExplorerFooter />
			</div>
		</ExplorerContext.Provider>
	)
}
