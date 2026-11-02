import { ReadingDirection, ReadingMode } from '@stump/graphql'
import { render } from '@testing-library/react'

import { useBookPreferences } from '@/scenes/book/reader/useBookPreferences'

import { EpubReaderControls, useEpubReaderContext, useEpubReaderControls } from '../../context'
import EpubNavigationControls from '../EpubNavigationControls'
import { pageTapZone, READER_PAGE_TAP_EVENT } from '../pageTap'

vi.mock('@stump/i18n', () => ({
	...vi.importActual('@stump/i18n'),
	useCreateTranslate: () => ({ t: (s: string) => s, translate: (s: string) => s }),
}))

vi.mock('../../context', () => ({
	...vi.importActual('../../context'),
	useEpubReaderContext: vi.fn(),
	useEpubReaderControls: vi.fn(),
}))

vi.mock('@/scenes/book/reader/useBookPreferences', () => ({
	useBookPreferences: vi.fn(),
}))

describe('pageTapZone', () => {
	it('splits the page into a 15% strip at each side and the middle', () => {
		expect(pageTapZone(0.05)).toBe('left')
		expect(pageTapZone(0.5)).toBe('middle')
		expect(pageTapZone(0.84)).toBe('middle')
		expect(pageTapZone(0.9)).toBe('right')
	})
})

describe('EpubNavigationControls page taps', () => {
	const controls = {
		visible: false,
		setVisible: vi.fn(),
		toggleVisible: vi.fn(),
		onPaginateForward: vi.fn(),
		onPaginateBackward: vi.fn(),
		canGoForward: true,
		canGoBackward: true,
	}

	function setup({ phone = true, rtl = false } = {}) {
		vi.mocked(useEpubReaderContext).mockReturnValue({
			readerMeta: { bookEntity: { id: 'book' } },
		} as never)
		vi.mocked(useEpubReaderControls).mockReturnValue(controls as unknown as EpubReaderControls)
		vi.mocked(useBookPreferences).mockReturnValue({
			bookPreferences: {
				readingDirection: rtl ? ReadingDirection.Rtl : ReadingDirection.Ltr,
				readingMode: ReadingMode.Paged,
				tapSidesToNavigate: true,
			},
		} as never)
		window.matchMedia = vi.fn().mockReturnValue({ matches: phone }) as never

		const { getByTestId } = render(
			<EpubNavigationControls>
				<div data-testid="page" />
			</EpubNavigationControls>,
		)
		const page = getByTestId('page')
		return (xFraction: number) =>
			page.dispatchEvent(
				new CustomEvent(READER_PAGE_TAP_EVENT, { bubbles: true, detail: { xFraction } }),
			)
	}

	beforeEach(() => vi.clearAllMocks())

	it('turns the page from the side strips and toggles the controls from the middle on phones', () => {
		const tap = setup()

		tap(0.95)
		expect(controls.onPaginateForward).toHaveBeenCalledTimes(1)
		tap(0.02)
		expect(controls.onPaginateBackward).toHaveBeenCalledTimes(1)
		tap(0.5)
		expect(controls.toggleVisible).toHaveBeenCalled()
	})

	it('mirrors the side strips for right-to-left books', () => {
		const tap = setup({ rtl: true })

		tap(0.95)
		expect(controls.onPaginateBackward).toHaveBeenCalledTimes(1)
		expect(controls.onPaginateForward).not.toHaveBeenCalled()
	})

	it('ignores page clicks on wider screens, which have chevrons', () => {
		const tap = setup({ phone: false })

		tap(0.95)
		tap(0.5)
		expect(controls.onPaginateForward).not.toHaveBeenCalled()
		expect(controls.toggleVisible).not.toHaveBeenCalled()
	})

	it('leaves the page uncovered: no tap zone sits over the text', () => {
		setup()
		const buttons = Array.from(document.querySelectorAll('button'))
		const coveringButtons = buttons.filter(
			(button) => !button.className.includes('sr-only') && !button.closest('.hidden'),
		)
		expect(coveringButtons).toHaveLength(0)
	})
})
