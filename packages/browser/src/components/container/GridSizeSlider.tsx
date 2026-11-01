import { Slider } from '@stump/components'
import { ZoomIn, ZoomOut } from 'lucide-react'

import { useGridSize } from './useGridSize'

export function GridSizeSlider() {
	const { density, setDensity, availableDensities } = useGridSize()
	const currentIndex = availableDensities.indexOf(density)

	const handleValueChange = (value: number[]) => {
		const newIndex = value[0]
		if (newIndex != null && newIndex >= 0 && newIndex < availableDensities.length) {
			const selectedDensity = availableDensities[newIndex]
			if (selectedDensity) {
				setDensity(selectedDensity)
			}
		}
	}

	// TODO: Tooltip as you drag that shows a label, e.g. below but from locale files
	return (
		// Zoom icons, not grid ones: next to the grid/table toggle, grid icons read as a second
		// pair of layout buttons. Left gives fewer, bigger cards; right more, smaller ones
		<div className="gap-2 flex items-center">
			<ZoomIn aria-hidden className="h-4 w-4 text-muted-foreground" />
			<Slider
				value={[currentIndex]}
				onValueChange={handleValueChange}
				max={availableDensities.length - 1}
				min={0}
				step={1}
				className="w-24"
			/>
			<ZoomOut aria-hidden className="h-4 w-4 text-muted-foreground" />
		</div>
	)
}

// const DENSITY_LABELS: Record<GridDensity, string> = {
// 	'ultra-compact': 'Ultra Compact',
// 	compact: 'Compact',
// 	cozy: 'Cozy',
// 	comfortable: 'Comfortable',
// 	spacious: 'Spacious',
// 	expansive: 'Expansive',
// }
