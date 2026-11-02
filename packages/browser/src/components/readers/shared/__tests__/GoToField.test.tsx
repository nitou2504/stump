import { LocaleProvider } from '@stump/i18n'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import GoToField, { resolvePlaceInput } from '../GoToField'

describe('resolvePlaceInput', () => {
	it('takes a page or location number inside the book', () => {
		expect(resolvePlaceInput('1', 315)).toBe(1)
		expect(resolvePlaceInput(' 123 ', 315)).toBe(123)
		expect(resolvePlaceInput('315', 315)).toBe(315)
		expect(resolvePlaceInput('0', 315)).toBeNull()
		expect(resolvePlaceInput('316', 315)).toBeNull()
	})

	it('takes a share of the book', () => {
		expect(resolvePlaceInput('0%', 315)).toBe(1)
		expect(resolvePlaceInput('50%', 315)).toBe(158)
		expect(resolvePlaceInput('100 %', 315)).toBe(315)
		expect(resolvePlaceInput('12,5%', 9)).toBe(2)
		expect(resolvePlaceInput('120%', 315)).toBeNull()
	})

	it('refuses anything else', () => {
		expect(resolvePlaceInput('', 315)).toBeNull()
		expect(resolvePlaceInput('xix', 315)).toBeNull()
		expect(resolvePlaceInput('-3', 315)).toBeNull()
		expect(resolvePlaceInput('3', 0)).toBeNull()
	})
})

describe('GoToField', () => {
	const renderField = (onGo: (place: number) => void) =>
		render(
			<LocaleProvider locale="en-US">
				<GoToField
					label="Go to page"
					placeholder="1 – 20"
					resolve={(input) => resolvePlaceInput(input, 20)}
					invalidMessage="No such page"
					onGo={onGo}
				/>
			</LocaleProvider>,
		)

	it('goes to what was typed on Enter', async () => {
		const onGo = vi.fn()
		renderField(onGo)
		await userEvent.type(screen.getByRole('textbox', { name: 'Go to page' }), '12{Enter}')
		expect(onGo).toHaveBeenCalledWith(12)
	})

	it('says so when the page does not exist', async () => {
		const onGo = vi.fn()
		renderField(onGo)
		await userEvent.type(screen.getByRole('textbox', { name: 'Go to page' }), '99{Enter}')
		expect(onGo).not.toHaveBeenCalled()
		expect(screen.getByText('No such page')).toBeInTheDocument()
	})
})
