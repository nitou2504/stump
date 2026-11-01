import { renderHook } from '@testing-library/react'
import { useLocation, useNavigate, useNavigationType } from 'react-router'

import { planLeave, useLeavePages, useRecordHistoryTrail } from '../useHistoryTrail'

vi.mock('react-router', () => ({
	useLocation: vi.fn(),
	useNavigate: vi.fn(),
	useNavigationType: vi.fn(),
}))

const isBookPage = (path: string) => path === '/books/b1' || path.startsWith('/books/b1/')

describe('planLeave', () => {
	it('goes back to the page the book was opened from', () => {
		const trail = { 0: '/', 1: '/libraries/l1/books?search=dune', 2: '/books/b1' }
		expect(planLeave(trail, 2, isBookPage)).toEqual({ back: 1, replace: false })
	})

	it('skips the other pages of the book, like its reader or settings', () => {
		const trail = {
			0: '/browse/authors?q=frank',
			1: '/books/b1',
			2: '/books/b1/epub-reader?',
			3: '/books/b1/manage',
		}
		expect(planLeave(trail, 3, isBookPage)).toEqual({ back: 3, replace: false })
	})

	it('replaces the book pages when nothing before them is known', () => {
		expect(planLeave({ 0: '/books/b1' }, 0, isBookPage)).toEqual({ back: 0, replace: true })
		expect(planLeave({ 0: '/books/b1', 1: '/books/b1/manage' }, 1, isBookPage)).toEqual({
			back: 1,
			replace: true,
		})
		// an entry from before the trail was recorded could be anything, even another site
		expect(planLeave({ 1: '/books/b1', 2: '/books/b1/manage' }, 2, isBookPage)).toEqual({
			back: 1,
			replace: true,
		})
	})

	it('treats other books as pages to go back to', () => {
		const trail = { 0: '/books/b10', 1: '/books/b1' }
		expect(planLeave(trail, 1, isBookPage)).toEqual({ back: 1, replace: false })
	})
})

describe('useRecordHistoryTrail', () => {
	beforeEach(() => sessionStorage.clear())

	const visit = (index: number, pathname: string, search: string, type: string) => {
		window.history.replaceState({ idx: index }, '')
		vi.mocked(useLocation).mockReturnValue({
			hash: '',
			key: `${index}${pathname}${search}`,
			pathname,
			search,
			state: null,
		})
		vi.mocked(useNavigationType).mockReturnValue(type as ReturnType<typeof useNavigationType>)
		renderHook(() => useRecordHistoryTrail())
	}
	const recorded = () => JSON.parse(sessionStorage.getItem('stump-history-trail') ?? '{}')

	it('records the page of each entry, with its query', () => {
		visit(0, '/', '', 'POP')
		visit(1, '/libraries/l1/books', '?search=dune', 'PUSH')
		visit(2, '/books/b1', '', 'PUSH')
		visit(1, '/libraries/l1/books', '?search=dune', 'POP')

		expect(recorded()).toEqual({ 0: '/', 1: '/libraries/l1/books?search=dune', 2: '/books/b1' })
	})

	it('updates replaced entries and forgets the ones ahead of a new entry', () => {
		visit(0, '/', '', 'POP')
		visit(1, '/books/b1', '', 'PUSH')
		visit(2, '/books/b1/manage', '', 'PUSH')
		visit(1, '/books/b1', '', 'POP')
		visit(1, '/books/b1', '?tab=info', 'REPLACE')
		visit(2, '/browse/authors', '', 'PUSH')

		expect(recorded()).toEqual({ 0: '/', 1: '/books/b1?tab=info', 2: '/browse/authors' })
	})

	it('ignores entries React Router did not number', () => {
		window.history.replaceState(null, '')
		vi.mocked(useLocation).mockReturnValue({
			hash: '',
			key: 'default',
			pathname: '/',
			search: '',
			state: null,
		})
		vi.mocked(useNavigationType).mockReturnValue('POP' as ReturnType<typeof useNavigationType>)
		renderHook(() => useRecordHistoryTrail())

		expect(recorded()).toEqual({})
	})
})

describe('useLeavePages', () => {
	const navigate = vi.fn()

	beforeEach(() => {
		navigate.mockReset()
		vi.mocked(useNavigate).mockReturnValue(navigate)
		sessionStorage.clear()
	})

	/** The browser shows the last page of `trail`, at history entry `index` */
	const setHistory = (index: number | undefined, trail: Record<number, string>) => {
		const current = index === undefined ? '/books/b1' : trail[index]
		window.history.replaceState(index === undefined ? null : { idx: index }, '', current)
		sessionStorage.setItem('stump-history-trail', JSON.stringify(trail))
	}

	it('navigates back through the history', () => {
		setHistory(3, { 0: '/', 1: '/smart-lists/s1', 2: '/books/b1', 3: '/books/b1/manage' })
		const { result } = renderHook(() => useLeavePages())

		result.current(isBookPage, '/series/s1/books')

		expect(navigate).toHaveBeenCalledExactlyOnceWith(-2)
	})

	it('replaces the page with the fallback without any history', () => {
		setHistory(undefined, {})
		const { result } = renderHook(() => useLeavePages())

		result.current(isBookPage, '/series/s1/books')

		expect(navigate).toHaveBeenCalledExactlyOnceWith('/series/s1/books', { replace: true })
	})

	it('stays on a page that is not being left', () => {
		setHistory(2, { 0: '/', 1: '/books/b1', 2: '/libraries/l1/books' })
		const { result } = renderHook(() => useLeavePages())

		result.current(isBookPage, '/series/s1/books')

		expect(navigate).not.toHaveBeenCalled()
	})

	it('goes back to the first book page, then replaces it with the fallback', () => {
		setHistory(1, { 0: '/books/b1', 1: '/books/b1/manage' })
		const { result } = renderHook(() => useLeavePages())

		result.current(isBookPage, '/libraries/l1/books')

		expect(navigate).toHaveBeenCalledExactlyOnceWith(-1)
		window.dispatchEvent(new PopStateEvent('popstate'))
		expect(navigate).toHaveBeenLastCalledWith('/libraries/l1/books', { replace: true })
		expect(navigate).toHaveBeenCalledTimes(2)
	})
})
