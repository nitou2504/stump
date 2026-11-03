import { DEFAULT_EPUB_FONT_SIZE } from '@stump/client'
import { Input } from '@stump/components'
import React from 'react'

import { useTranslate } from '@/hooks/useTranslate'

import { useEpubDefault } from './useEpubDefault'

export default function DefaultFontSize() {
	const { translate } = useTranslate()
	const [fontSize, setFontSize] = useEpubDefault('fontSize')

	const onValueChange = (e: React.ChangeEvent<HTMLInputElement>) => {
		const value = parseInt(e.target.value)
		if (!isNaN(value) && value > 0) {
			setFontSize(value)
		}
	}

	return (
		<div className="py-1.5">
			<Input
				label={translate('shared.epubSettings.fontSize.label')}
				description={translate('shared.epubSettings.fontSize.description')}
				value={fontSize ?? DEFAULT_EPUB_FONT_SIZE}
				onChange={onValueChange}
				type="number"
				min={0}
			/>
		</div>
	)
}
