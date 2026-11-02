import { AnnotationStyle } from '@stump/graphql'
import { fireEvent, render, screen } from '@testing-library/react'

import AnnotationToolbar, { computeToolbarPosition } from '../AnnotationToolbar'

vi.mock('@stump/i18n', () => ({
	...vi.importActual('@stump/i18n'),
	useCreateTranslate: () => ({ t: (s: string) => s, translate: (s: string) => s }),
}))

const VIEWPORT = { width: 400, height: 800 }
const SIZE = { width: 200, height: 40 }

describe('computeToolbarPosition', () => {
	it('centres the toolbar above the selection', () => {
		const anchor = { x: 100, y: 300, width: 100, height: 20 }
		expect(computeToolbarPosition(anchor, SIZE, VIEWPORT)).toEqual({
			left: 50,
			top: 300 - 8 - 40,
			placement: 'above',
		})
	})

	it('goes below when there is no room above, or when preferred (touch screens)', () => {
		const nearTop = { x: 100, y: 20, width: 100, height: 20 }
		expect(computeToolbarPosition(nearTop, SIZE, VIEWPORT)).toMatchObject({
			top: 20 + 20 + 8,
			placement: 'below',
		})

		const middle = { x: 100, y: 300, width: 100, height: 60 }
		expect(computeToolbarPosition(middle, SIZE, VIEWPORT, { preferBelow: true })).toMatchObject({
			top: 300 + 60 + 8,
			placement: 'below',
		})
	})

	it('never leaves the viewport', () => {
		const rightEdge = { x: 390, y: 300, width: 10, height: 20 }
		expect(computeToolbarPosition(rightEdge, SIZE, VIEWPORT).left).toBe(400 - 200 - 8)

		const leftEdge = { x: 0, y: 300, width: 10, height: 20 }
		expect(computeToolbarPosition(leftEdge, SIZE, VIEWPORT).left).toBe(8)

		// a selection taller than the screen: pinned inside it
		const huge = { x: 0, y: 0, width: 400, height: 800 }
		const { top } = computeToolbarPosition(huge, SIZE, VIEWPORT)
		expect(top).toBeGreaterThanOrEqual(8)
		expect(top + SIZE.height).toBeLessThanOrEqual(800 - 8)
	})
})

describe('AnnotationToolbar', () => {
	const anchor = { x: 100, y: 300, width: 100, height: 20 }

	it('shows every style, the note and copy at once for a selection', () => {
		const onStyle = vi.fn()
		const onNote = vi.fn()
		const onCopy = vi.fn()
		render(
			<AnnotationToolbar
				anchor={anchor}
				mode="selection"
				onStyle={onStyle}
				onNote={onNote}
				onCopy={onCopy}
			/>,
		)

		// no 'Annotate' step: every glyph is there straight away, in toolbar order
		expect(screen.queryByLabelText('shared.readerAnnotations.annotate')).toBeNull()
		const labels = screen.getAllByRole('button').map((button) => button.getAttribute('aria-label'))
		expect(labels).toEqual([
			'shared.readerAnnotations.styles.highlight',
			'shared.readerAnnotations.styles.underline',
			'shared.readerAnnotations.styles.strikethrough',
			'shared.readerAnnotations.styles.invert',
			'shared.readerAnnotations.addNote',
			'shared.readerAnnotations.copy',
		])

		fireEvent.click(screen.getByLabelText('shared.readerAnnotations.styles.highlight'))
		expect(onStyle).toHaveBeenCalledWith(AnnotationStyle.Highlight)
		fireEvent.click(screen.getByLabelText('shared.readerAnnotations.styles.invert'))
		expect(onStyle).toHaveBeenLastCalledWith(AnnotationStyle.Invert)
		fireEvent.click(screen.getByLabelText('shared.readerAnnotations.addNote'))
		expect(onNote).toHaveBeenCalled()
		fireEvent.click(screen.getByLabelText('shared.readerAnnotations.copy'))
		expect(onCopy).toHaveBeenCalled()
		// tooltips name the shortcut
		expect(screen.getByLabelText('shared.readerAnnotations.styles.underline')).toHaveAttribute(
			'title',
			'shared.readerAnnotations.styles.underline (U)',
		)
		expect(screen.getByLabelText('shared.readerAnnotations.copy')).toHaveAttribute(
			'title',
			'shared.readerAnnotations.copy (C)',
		)
		// a selection has nothing to delete
		expect(screen.queryByLabelText('shared.readerAnnotations.delete')).toBeNull()
	})

	it('shows every style (the current one pressed), the note and delete for an annotation', () => {
		const onDelete = vi.fn()
		render(
			<AnnotationToolbar
				anchor={anchor}
				mode="annotation"
				activeStyle={AnnotationStyle.Underline}
				hasNote
				onStyle={vi.fn()}
				onNote={vi.fn()}
				onDelete={onDelete}
			/>,
		)

		expect(screen.getByLabelText('shared.readerAnnotations.styles.underline')).toHaveAttribute(
			'aria-pressed',
			'true',
		)
		expect(screen.getByLabelText('shared.readerAnnotations.styles.highlight')).toHaveAttribute(
			'aria-pressed',
			'false',
		)
		expect(screen.getByLabelText('shared.readerAnnotations.editNote')).toBeInTheDocument()
		fireEvent.click(screen.getByLabelText('shared.readerAnnotations.delete'))
		expect(onDelete).toHaveBeenCalled()
	})

	it("doesn't take the focus (so the text selection survives)", () => {
		render(
			<AnnotationToolbar anchor={anchor} mode="selection" onStyle={vi.fn()} onNote={vi.fn()} />,
		)
		const event = new MouseEvent('mousedown', { bubbles: true, cancelable: true })
		screen.getByRole('toolbar').dispatchEvent(event)
		expect(event.defaultPrevented).toBe(true)
	})
})
