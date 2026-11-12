import { useFooterOffsetStore } from '@stump/client'
import { cn } from '@stump/components'
import { forwardRef, Suspense, useEffect } from 'react'

import { TablePaginationProps } from '@/components/table'

import URLPageSize from './URLPageSize'
import URLPagination from './URLPagination'

type Props = {
	tableControls?: React.ReactNode
	/** No page controls, for a list that fits on one page (the table's controls need the footer) */
	hideFooter?: boolean
} & TablePaginationProps &
	Pick<React.HTMLAttributes<HTMLDivElement>, 'className' | 'children'>

const URLFilterContainer = forwardRef<HTMLDivElement, Props>(
	({ children, className, tableControls, hideFooter, ...paginationProps }, ref) => {
		const storeOffset = useFooterOffsetStore((state) => state.setFooterOffset)

		useEffect(() => {
			storeOffset(hideFooter ? 0 : 48)
			return () => {
				storeOffset(0)
			}
		}, [storeOffset, hideFooter])

		return (
			<Suspense>
				<div ref={ref} className={cn('flex flex-1 flex-col', className)} id="urlFilterContainer">
					<div className="flex flex-1 flex-col overflow-x-clip">{children}</div>
					{!hideFooter && (
						<div
							className="bottom-0 h-12 px-4 md:h-10 sticky z-50 flex w-full shrink-0 items-center justify-between border-t border-border bg-background"
							data-testid="urlFilterFooter"
						>
							<div className="gap-x-2 flex shrink-0 items-center">
								{tableControls}
								<URLPageSize />
							</div>
							<URLPagination {...paginationProps} />
						</div>
					)}
				</div>
			</Suspense>
		)
	},
)
URLFilterContainer.displayName = 'URLFilterContainer'

export default URLFilterContainer
