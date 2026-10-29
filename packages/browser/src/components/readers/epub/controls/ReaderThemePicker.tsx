import { EPUB_READER_THEMES, type EpubReaderTheme, type EpubTextAlign } from '@stump/client'
import { cn, Label, NativeSelect } from '@stump/components'
import { Check } from 'lucide-react'

import { useTheme } from '@/hooks/useTheme'
import { useReaderStore } from '@/stores'

import { resolveReaderTheme } from '../readium/preferences'

const THEME_LABELS: Record<EpubReaderTheme, string> = {
	auto: 'Auto',
	light: 'Light',
	sepia: 'Sepia',
	dark: 'Dark',
	black: 'Black',
}

/**
 * Reader colour theme swatches and text alignment. Both are global reader settings, so
 * they follow you from book to book.
 */
export default function ReaderThemePicker() {
	const { isDarkVariant } = useTheme()
	const theme = useReaderStore((state) => state.settings.theme ?? 'auto')
	const textAlign = useReaderStore((state) => state.settings.textAlign ?? 'auto')
	const setSettings = useReaderStore((state) => state.setSettings)

	return (
		<div className="gap-3 py-1.5 flex flex-col">
			<div>
				<Label>Theme</Label>
				<div className="mt-1.5 gap-2 flex flex-wrap" role="radiogroup" aria-label="Reader theme">
					{EPUB_READER_THEMES.map((option) => {
						const colors = resolveReaderTheme(option, isDarkVariant)
						const isActive = theme === option
						return (
							<button
								key={option}
								type="button"
								role="radio"
								aria-checked={isActive}
								title={option === 'auto' ? 'Follow the app theme' : THEME_LABELS[option]}
								onClick={() => setSettings({ theme: option })}
								className={cn(
									'gap-1.5 px-3 py-1.5 text-sm flex items-center rounded-lg border-2 transition-colors',
									isActive ? 'border-primary' : 'border-border hover:border-muted-foreground',
								)}
								style={{ backgroundColor: colors.background, color: colors.text }}
							>
								{isActive && <Check className="h-3.5 w-3.5" />}
								{option === 'auto' ? 'Aa Auto' : `Aa ${THEME_LABELS[option]}`}
							</button>
						)
					})}
				</div>
			</div>

			<div>
				<Label htmlFor="text-align">Text alignment</Label>
				<NativeSelect
					id="text-align"
					size="sm"
					className="mt-1.5"
					value={textAlign}
					onChange={(e) => setSettings({ textAlign: e.target.value as EpubTextAlign })}
					options={[
						{ label: "Book's default", value: 'auto' },
						{ label: 'Justified (with hyphenation)', value: 'justify' },
						{ label: 'Left aligned', value: 'start' },
					]}
				/>
			</div>
		</div>
	)
}
