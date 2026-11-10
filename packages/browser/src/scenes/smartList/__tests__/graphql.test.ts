import type { Api } from '@stump/sdk'
import { describe, expect, it, vi } from 'vitest'

import { fetchSmartListItems, mergeSmartListPages, SMART_LIST_PAGE_SIZE } from '../graphql'

type Page = Parameters<typeof mergeSmartListPages>[0][number]

// What the server sends for a book is more than an id, but the id is all these tests look at
const books = (ids: string[]) => ids.map((id) => ({ id }))
const ungrouped = (ids: string[]) =>
	({ __typename: 'SmartListUngrouped', books: books(ids) }) as unknown as Page
const grouped = (groups: Array<[series: string | null, ids: string[]]>) =>
	({
		__typename: 'SmartListGrouped',
		items: groups.map(([series, ids]) => ({
			entity: series ? { __typename: 'Series', id: series, name: series } : null,
			books: books(ids),
		})),
	}) as unknown as Page

const ids = (list: Array<{ id: string }>) => list.map(({ id }) => id)
const range = (from: number, to: number) =>
	Array.from({ length: Math.max(to - from, 0) }, (_, i) => `book-${from + i}`)

describe('mergeSmartListPages', () => {
	it('puts the books of an ungrouped list one after the other', () => {
		const merged = mergeSmartListPages([ungrouped(['a', 'b']), ungrouped(['c'])])
		expect(merged.__typename).toBe('SmartListUngrouped')
		expect(ids((merged as unknown as { books: Array<{ id: string }> }).books)).toEqual([
			'a',
			'b',
			'c',
		])
	})

	it('shows a book once, even when two pages have it', () => {
		const merged = mergeSmartListPages([ungrouped(['a', 'b']), ungrouped(['b', 'c'])])
		expect(ids((merged as unknown as { books: Array<{ id: string }> }).books)).toEqual([
			'a',
			'b',
			'c',
		])
	})

	it('puts a group that runs over two pages back together', () => {
		const merged = mergeSmartListPages([
			grouped([
				['s1', ['a', 'b']],
				[null, ['x']],
			]),
			grouped([
				['s2', ['c']],
				['s1', ['d']],
				[null, ['y']],
			]),
		])
		expect(merged.__typename).toBe('SmartListGrouped')
		const items = (
			merged as unknown as {
				items: Array<{ entity: { id: string } | null; books: Array<{ id: string }> }>
			}
		).items
		// the groups in the order they came, the books of a group in the order they came
		expect(items.map((item) => [item.entity?.id ?? null, ids(item.books)])).toEqual([
			['s1', ['a', 'b', 'd']],
			[null, ['x', 'y']],
			['s2', ['c']],
		])
	})

	it('keeps a series and a library with the same id apart', () => {
		const library = {
			__typename: 'SmartListGrouped',
			items: [{ entity: { __typename: 'Library', id: 's1', name: 'l' }, books: books(['z']) }],
		} as unknown as Page
		const merged = mergeSmartListPages([grouped([['s1', ['a']]]), library])
		expect((merged as unknown as { items: unknown[] }).items).toHaveLength(2)
	})
})

describe('fetchSmartListItems', () => {
	/** A server with `total` books that hands out pages of the size asked for */
	const server = (total: number, page: (ids: string[]) => Page = ungrouped) => {
		const execute = vi.fn(
			async (_document: unknown, variables: { limit: number; offset: number }) => ({
				smartListItems: page(
					range(variables.offset, Math.min(total, variables.offset + variables.limit)),
				),
			}),
		)
		return { sdk: { execute } as unknown as Api, execute }
	}
	const offsets = (execute: ReturnType<typeof server>['execute']) =>
		execute.mock.calls.map(([, variables]) => variables.offset)

	it('reads a long list page by page, until a page that is not full', async () => {
		const { sdk, execute } = server(250)
		const items = await fetchSmartListItems(sdk, { id: 'list' })

		expect(offsets(execute)).toEqual([0, SMART_LIST_PAGE_SIZE, 2 * SMART_LIST_PAGE_SIZE])
		const everything = (items as unknown as { books: Array<{ id: string }> }).books
		expect(ids(everything)).toEqual(range(0, 250))
	})

	it('asks once more when the last page is full, and stops at the empty one', async () => {
		const { sdk, execute } = server(2 * SMART_LIST_PAGE_SIZE)
		const items = await fetchSmartListItems(sdk, { id: 'list' })

		expect(offsets(execute)).toEqual([0, SMART_LIST_PAGE_SIZE, 2 * SMART_LIST_PAGE_SIZE])
		expect((items as unknown as { books: unknown[] }).books).toHaveLength(2 * SMART_LIST_PAGE_SIZE)
	})

	it('reads a short list with one request, and an empty one too', async () => {
		const short = server(5)
		await fetchSmartListItems(short.sdk, { id: 'list' })
		expect(short.execute).toHaveBeenCalledTimes(1)

		const empty = server(0)
		const items = await fetchSmartListItems(empty.sdk, { id: 'list' })
		expect(empty.execute).toHaveBeenCalledTimes(1)
		expect((items as unknown as { books: unknown[] }).books).toEqual([])
	})

	it('asks for pages of the size the server prices, never more than 200 books', async () => {
		const { sdk, execute } = server(10)
		await fetchSmartListItems(sdk, { id: 'list' })
		expect(execute.mock.calls[0]?.[1].limit).toBe(SMART_LIST_PAGE_SIZE)
		expect(SMART_LIST_PAGE_SIZE).toBeLessThanOrEqual(200)
	})

	it('passes the filter and the grouping on to every page', async () => {
		const { sdk, execute } = server(150)
		await fetchSmartListItems(sdk, {
			id: 'list',
			filter: { name: { contains: 'x' } },
			grouping: 'BY_SERIES' as never,
		})
		expect(execute).toHaveBeenCalledTimes(2)
		for (const [, variables] of execute.mock.calls) {
			expect(variables).toMatchObject({
				id: 'list',
				filter: { name: { contains: 'x' } },
				grouping: 'BY_SERIES',
			})
		}
	})

	it('counts the books of a grouped page, over all its groups', async () => {
		// 100 books in 4 groups is a full page, so the next one is asked for
		const full = (ids: string[]) =>
			grouped([
				['s1', ids.slice(0, 25)],
				['s2', ids.slice(25, 50)],
				['s3', ids.slice(50, 75)],
				[null, ids.slice(75)],
			])
		const { sdk, execute } = server(130, full)
		await fetchSmartListItems(sdk, { id: 'list' })
		expect(offsets(execute)).toEqual([0, SMART_LIST_PAGE_SIZE])
	})
})
