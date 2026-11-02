import { Dropdown, IconButton } from '@stump/components'
import { MoreHorizontal, TableProperties } from 'lucide-react'
import { useState } from 'react'

import { useSavedViews } from './SavedViewSelector'
import TableColumnsBottomDrawer from './TableColumnsBottomDrawer'

type Props = {
	/** Whether the list has saved views to pick from */
	hasSavedViews: boolean
	/** Whether the table's columns can be adjusted (table layout only) */
	showColumnControls: boolean
}

/**
 * A smart list's less frequent controls on a phone, where the toolbar is one row: the saved
 * views and the table's columns. Renders nothing when neither applies.
 */
export default function SmartListMoreMenu({ hasSavedViews, showColumnControls }: Props) {
	const [isColumnsOpen, setIsColumnsOpen] = useState(false)
	const { currentValue, emptyOption, options, selectView, translateKey } = useSavedViews()

	if (!hasSavedViews && !showColumnControls) return null

	const viewOptions = emptyOption ? [emptyOption, ...options] : options

	return (
		<>
			<Dropdown modal={false}>
				<Dropdown.Trigger asChild>
					<IconButton
						variant="ghost"
						size="sm"
						className="shrink-0 hover:bg-accent data-[state=open]:bg-accent"
						aria-label="More options"
					>
						<MoreHorizontal className="h-4 w-4" />
					</IconButton>
				</Dropdown.Trigger>

				<Dropdown.Content align="end" className="min-w-52">
					{hasSavedViews && (
						<>
							<Dropdown.Label>{translateKey('selectView')}</Dropdown.Label>
							<Dropdown.RadioGroup value={currentValue} onValueChange={selectView}>
								{viewOptions.map((option) => (
									<Dropdown.RadioItem key={option.value || 'default'} value={option.value}>
										{option.label}
									</Dropdown.RadioItem>
								))}
							</Dropdown.RadioGroup>
						</>
					)}

					{hasSavedViews && showColumnControls && <Dropdown.Separator />}

					{showColumnControls && (
						<Dropdown.Item onSelect={() => setIsColumnsOpen(true)}>
							<TableProperties className="h-4 w-4" />
							Adjust columns
						</Dropdown.Item>
					)}
				</Dropdown.Content>
			</Dropdown>

			{showColumnControls && (
				<TableColumnsBottomDrawer open={isColumnsOpen} onOpenChange={setIsColumnsOpen} />
			)}
		</>
	)
}
