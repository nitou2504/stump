import CleanLibrary from './CleanLibrary'
import DeleteLibrary from './DeleteLibrary'
import RemovedBooks from './RemovedBooks'

export default function DeletionScene() {
	return (
		<div className="gap-12 flex flex-col">
			<CleanLibrary />
			<RemovedBooks />
			<DeleteLibrary />
		</div>
	)
}
