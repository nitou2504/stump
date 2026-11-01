import { Locator, LocatorLocations } from '@readium/shared'
import { act, renderHook } from '@testing-library/react'
import { MemoryRouter, useLocation, useNavigate } from 'react-router'

import { linkDepthOf, useLinkReturnStack } from '../useLinkReturnStack'

const place = (position: number) =>
	new Locator({
		href: `OEBPS/chapter${position}.xhtml`,
		type: 'application/xhtml+xml',
		locations: new LocatorLocations({ position }),
	})

function setup() {
	let here: Locator = place(1)
	const goTo = vi.fn(async (locator: Locator) => {
		here = locator
		return true
	})
	const view = renderHook(
		() => ({
			stack: useLinkReturnStack({ goTo, current: () => here }),
			location: useLocation(),
			navigate: useNavigate(),
		}),
		{
			wrapper: ({ children }) => (
				<MemoryRouter initialEntries={['/books/1/epub-reader?x=1']}>{children}</MemoryRouter>
			),
		},
	)
	return { ...view, goTo, where: () => here }
}

describe('useLinkReturnStack', () => {
	it('follows a link in a new history entry of the same page', async () => {
		const { result, goTo } = setup()
		expect(result.current.stack.canReturn).toBe(false)

		await act(() => result.current.stack.follow(place(5)))

		expect(goTo).toHaveBeenLastCalledWith(place(5))
		expect(result.current.location.pathname).toBe('/books/1/epub-reader')
		expect(result.current.location.search).toBe('?x=1')
		expect(linkDepthOf(result.current.location.state)).toBe(1)
		expect(result.current.stack.canReturn).toBe(true)
	})

	it('returns to where the link was, with Return or the browser Back', async () => {
		const { result, where } = setup()
		await act(() => result.current.stack.follow(place(5)))
		await act(() => result.current.stack.follow(place(9)))
		expect(where().locations.position).toBe(9)

		// Return
		act(() => result.current.stack.returnBack())
		expect(where().locations.position).toBe(5)
		expect(linkDepthOf(result.current.location.state)).toBe(1)

		// Browser Back
		act(() => result.current.navigate(-1))
		expect(where().locations.position).toBe(1)
		expect(result.current.stack.canReturn).toBe(false)
	})

	it('does not jump again on Forward', async () => {
		const { result, goTo } = setup()
		await act(() => result.current.stack.follow(place(5)))
		act(() => result.current.navigate(-1))
		const calls = goTo.mock.calls.length

		act(() => result.current.navigate(1))
		expect(goTo.mock.calls.length).toBe(calls)
		expect(result.current.stack.canReturn).toBe(false)
	})

	it('reads the depth from history state', () => {
		expect(linkDepthOf(null)).toBe(0)
		expect(linkDepthOf({ epubLinkDepth: 3 })).toBe(3)
		expect(linkDepthOf({ epubLinkDepth: 'x' })).toBe(0)
	})
})
