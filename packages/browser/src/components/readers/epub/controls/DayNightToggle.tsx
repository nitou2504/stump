import { Moon, Sun } from 'lucide-react'

import { useTheme } from '@/hooks/useTheme'
import { useReaderStore } from '@/stores'

import ControlButton from './ControlButton'

const DARK_THEMES = ['dark', 'black']

/**
 * One-tap switch between the light and dark reader themes (sepia/black are in the theme
 * dialog). From `auto` it switches to the opposite of what is currently shown.
 */
export default function DayNightToggle() {
	const { isDarkVariant } = useTheme()
	const theme = useReaderStore((state) => state.settings.theme ?? 'auto')
	const setSettings = useReaderStore((state) => state.setSettings)

	const isDark = theme === 'auto' ? isDarkVariant : DARK_THEMES.includes(theme)

	return (
		<ControlButton
			title={isDark ? 'Day mode' : 'Night mode'}
			onClick={() => setSettings({ theme: isDark ? 'light' : 'dark' })}
		>
			{isDark ? <Sun className="h-4 w-4" /> : <Moon className="h-4 w-4" />}
		</ControlButton>
	)
}
