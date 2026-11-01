import { NewCard, Select } from '@stump/components'
import { InterfaceLayout } from '@stump/graphql'

import { useLayoutMode } from '@/hooks/useLayoutMode'
import { useTranslate } from '@/hooks/useTranslate'

/**
 * Grid or table for every book and series list. The same preference the grid/table buttons
 * of the lists change, so both always show the same value.
 */
export default function PreferredLayoutSelect() {
	const { t } = useTranslate()
	const { layoutMode, setLayoutMode } = useLayoutMode()

	return (
		<NewCard.Row label={t(getKey('label'))} description={t(getKey('description'))}>
			<div>
				<Select
					value={layoutMode}
					onValueChange={(value) =>
						setLayoutMode(
							value === InterfaceLayout.Table ? InterfaceLayout.Table : InterfaceLayout.Grid,
						)
					}
				>
					<Select.Trigger size="sm">
						<Select.Value />
					</Select.Trigger>
					<Select.Content>
						{[InterfaceLayout.Grid, InterfaceLayout.Table].map((option) => (
							<Select.Item key={option} value={option}>
								{t(getKey(`options.${option}`))}
							</Select.Item>
						))}
					</Select.Content>
				</Select>
			</div>
		</NewCard.Row>
	)
}

const getKey = (key: string) => `settingsScene.app/preferences.sections.preferredLayout.${key}`
