import { MiniStatCard } from '@stump/components'
import { render, screen } from '@testing-library/react'
import { Layers } from 'lucide-react'

describe('MiniStatCard', () => {
	it('reads as a number and its word ("813 series"), not run together', () => {
		render(<MiniStatCard icon={Layers} value="813" suffix="series" title="813 series" />)

		expect(screen.getByTitle('813 series')).toHaveTextContent('813 series')
		expect(screen.getByTitle('813 series').textContent).toBe('813 series')
	})

	it('shows a bare value as it is', () => {
		render(<MiniStatCard value={5} title="five" />)
		expect(screen.getByTitle('five').textContent).toBe('5')
	})
})
