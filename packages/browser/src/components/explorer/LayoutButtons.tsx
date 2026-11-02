import { cn } from '@stump/components'

import { LayoutToggle } from '../TableOrGridLayout'
import { useFileExplorerContext } from './context'

export default function LayoutButtons({ className }: { className?: string }) {
	const { layout, setLayout } = useFileExplorerContext()
	const isGrid = layout === 'grid'

	return (
		<div className={cn('flex shrink-0 items-center', className)}>
			<LayoutToggle isGrid={isGrid} onToggle={() => setLayout(isGrid ? 'table' : 'grid')} />
		</div>
	)
}
