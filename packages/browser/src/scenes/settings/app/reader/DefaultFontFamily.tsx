import { ComboBox, Label } from '@stump/components'
import { isSupportedFont } from '@stump/sdk'
import { useCallback } from 'react'

import { useTranslate } from '@/hooks/useTranslate'
import { SUPPORTED_FONT_OPTIONS } from '@/scenes/settings/app/preferences/FontSelect'

import { useEpubDefault } from './useEpubDefault'

export default function DefaultFontFamily() {
	const { translate } = useTranslate()
	const [fontFamily, setFontFamily] = useEpubDefault('fontFamily')

	const changeFont = useCallback(
		(font?: string) => {
			if (!font) {
				// the book's own font: '' (not unset), so it wins over the reader's own setting
				setFontFamily('')
			} else if (isSupportedFont(font) || isSupportedFont(font.toUpperCase())) {
				setFontFamily(font.toUpperCase())
			}
		},
		[setFontFamily],
	)

	return (
		<div className="py-1.5">
			<Label htmlFor="font-family">{translate('shared.epubSettings.typeface.label')}</Label>
			<ComboBox
				size="full"
				options={[{ value: '', label: 'Default', fontClassName: '' }].concat(
					SUPPORTED_FONT_OPTIONS,
				)}
				value={fontFamily ?? ''}
				// '' is the "Default" option: show it instead of "Select..."
				placeholder="Default"
				onChange={changeFont}
			/>
		</div>
	)
}
