import { Input } from '@stump/components'
import React from 'react'

import { useTranslate } from '@/hooks/useTranslate'

import { useEpubDefault } from './useEpubDefault'

export default function DefaultLineHeight() {
	const { translate } = useTranslate()
	const [lineHeight, setLineHeight] = useEpubDefault('lineHeight')

	const onValueChange = (e: React.ChangeEvent<HTMLInputElement>) => {
		const value = parseFloat(e.target.value)
		if (!isNaN(value) && value >= 1.0 && value <= 3.0) {
			setLineHeight(value)
		}
	}

	return (
		<div className="py-1.5">
			<Input
				label={translate('shared.epubSettings.lineHeight.label')}
				description={translate('shared.epubSettings.lineHeight.description')}
				value={lineHeight ?? 1.5}
				onChange={onValueChange}
				type="number"
				min={1.0}
				max={3.0}
				step={0.1}
			/>
		</div>
	)
}
