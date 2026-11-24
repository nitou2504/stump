import { render } from '@testing-library/react'
import { describe, expect, it } from 'vitest'

import { highlight } from '../highlight'

describe('highlight', () => {
	it('marks the words of the query, whatever their case', () => {
		const { container } = render(<p>{highlight('Quiet Mind, quiet life', 'quiet')}</p>)
		expect([...container.querySelectorAll('mark')].map((m) => m.textContent)).toEqual([
			'Quiet',
			'quiet',
		])
	})

	it('does not break on characters that mean something in a pattern', () => {
		const { container } = render(<p>{highlight('a (b) c', '(b) +')}</p>)
		expect(container.textContent).toBe('a (b) c')
	})

	it('returns the text as it is when there is nothing to mark', () => {
		expect(highlight('plain', 'x')).toBe('plain')
	})
})
