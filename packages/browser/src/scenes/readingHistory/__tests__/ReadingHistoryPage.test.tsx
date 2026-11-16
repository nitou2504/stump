import '@/__mocks__/resizeObserver'

import { act, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { ReactNode } from 'react'
import { Route, Routes } from 'react-router'

import { FINISHED_PAGE_SIZE } from '../graphql'
import ReadingHistoryScene from '../index'
import { ana, finishedRead, history, readingNow, sent, setup } from './support'

vi.mock('@/hooks/useTranslate', () => import('./translations'))
vi.mock('../availability', () => ({ MY_READING_AVAILABLE: true }))
// the header, the covers and the search are tested where they are; here what the page puts in them
vi.mock('@/components/sharedLayout', () => ({
	// a real <header> (the banner landmark): where the page puts its chip can then be asserted, not guessed
	CollectionSceneHeader: ({ title, endActions }: { title: string; endActions?: ReactNode }) => (
		<header>
			<h1>{title}</h1>
			{endActions}
		</header>
	),
}))
vi.mock('@/components/thumbnail', () => ({
	ThumbnailImage: ({ src, alt }: { src: string; alt?: string }) => <img src={src} alt={alt} />,
}))
vi.mock('@/components/filters', async () => {
	const { useEffect, useRef } = await import('react')
	return {
		Search: ({
			initialValue,
			placeholder,
			onChange,
		}: {
			initialValue?: string
			placeholder?: string
			onChange: (value: string) => void
		}) => {
			// like the real box, which reports its value once when it has mounted (its debounced value
			// starts as the initial one and the previous one doesn't exist yet), and again on a change
			const first = useRef({ value: initialValue ?? '', report: onChange })
			useEffect(() => {
				first.current.report(first.current.value)
			}, [])
			return (
				<input
					aria-label={placeholder}
					defaultValue={initialValue}
					onChange={(event) => onChange(event.target.value)}
				/>
			)
		},
	}
})

const dune = { id: 'dune', name: 'Dune', writers: ['Frank Herbert'] }
const emma = { id: 'emma', name: 'Emma', writers: ['Jane Austen', 'Someone Else'] }
const kim = { id: 'kim', name: 'Kim' }
const ulysses = { id: 'ulysses', name: 'Ulysses' }

function renderPage(answers: ReturnType<typeof history>, path = '/reading') {
	const { execute, Wrapper } = setup({ answers, path })
	render(
		<Routes>
			<Route path="/reading" element={<ReadingHistoryScene />} />
			<Route path="/reading/:userId" element={<ReadingHistoryScene />} />
		</Routes>,
		{ wrapper: Wrapper },
	)
	return { execute }
}

const finishedSection = () =>
	screen.findByRole('region', { name: 'readingHistory.finished.heading' })

/**
 * Lets the page finish what it does once it has mounted. The search box tells its first value
 * only after it is in the page, and what the page does with it (the address, asking for the
 * books again) comes a moment later, so the first look at the address would still see the old one
 */
const settle = () =>
	act(async () => {
		await new Promise((resolve) => setTimeout(resolve, 50))
	})

describe('ReadingHistoryPage', () => {
	describe("a person's own page", () => {
		it('shows what is being read now, with how far along and when it was last read', async () => {
			renderPage(
				history({
					reading: [
						readingNow(dune, 43, '2026-03-09', 2),
						readingNow(kim, null, '2026-03-01'),
						readingNow(emma, 100, '2026-02-02'),
					],
				}),
			)

			const now = await screen.findByRole('region', { name: 'readingHistory.now.heading' })
			// in the order the server gives: the one read last first
			expect(
				within(now)
					.getAllByRole('link')
					.map((link) => link.getAttribute('href')),
			).toEqual(['/books/dune', '/books/kim', '/books/emma'])
			expect(within(now).getByText('Dune')).toBeInTheDocument()
			expect(within(now).getByAltText('Dune')).toHaveAttribute('src', '/covers/dune')

			// the progress is a bar only when there is one to tell
			const bars = within(now).getAllByRole('progressbar')
			expect(bars.map((bar) => bar.getAttribute('aria-valuenow'))).toEqual(['43', '100'])
			expect(within(now).getByText('43%')).toBeInTheDocument()

			// the day it was read last, on the calendar of the reader; a book is read or not, so no
			// "2nd read" is ever said, even for an old record numbered 2
			expect(
				within(now).getByText(/readingHistory\.now\.lastRead date=Mar 9, 2026/),
			).not.toHaveTextContent('ordinal')
			expect(within(now).getByText(/date=Mar 1, 2026/)).not.toHaveTextContent('ordinal')
		})

		it('says there is nothing in progress', async () => {
			renderPage(history())
			expect(await screen.findByText('readingHistory.now.empty')).toBeInTheDocument()
		})

		it('lists the finished books in months, newest first, with their dates (no read numbers)', async () => {
			renderPage(
				history({
					years: [
						{ year: 2026, count: 3 },
						{ year: 2025, count: 1 },
					],
					finished: [
						finishedRead(dune, '2026-03-04'),
						finishedRead(emma, '2026-03-02', 2, '2026-02-20'),
						finishedRead(kim, '2025-12-30'),
					],
				}),
			)

			const section = await finishedSection()
			const march = await within(section).findByRole('region', { name: 'March 2026' })
			const december = within(section).getByRole('region', { name: 'December 2025' })
			expect(
				within(march)
					.getAllByRole('link')
					.map((link) => link.textContent),
			).toHaveLength(2)
			expect(within(march).getByRole('heading')).toHaveTextContent('March 20262')
			expect(within(december).getAllByRole('link')).toHaveLength(1)

			// the book, who wrote it, when it was finished and, for a re-read, which read
			const emmaRow = within(march).getByRole('link', { name: /Emma/ })
			expect(emmaRow).toHaveAttribute('href', '/books/emma')
			expect(emmaRow).toHaveTextContent('Jane Austen, Someone Else')
			expect(emmaRow).toHaveTextContent('Mar 2, 2026')
			// an old record numbered 2 is not called a "2nd read": a book is read or not
			expect(emmaRow).not.toHaveTextContent('ordinal')
			const duneRow = within(march).getByRole('link', { name: /Dune/ })
			expect(duneRow).toHaveTextContent('Mar 4, 2026')
			expect(duneRow).not.toHaveTextContent('ordinal')

			// months are runs of the order the server gives
			const headings = within(section)
				.getAllByRole('heading', { level: 3 })
				.map((heading) => heading.textContent)
			expect(headings).toEqual(['March 20262', 'December 20251'])
			expect(within(section).getByText('readingHistory.finished.count count=4')).toBeInTheDocument()
		})

		it('shows the year chips with how many books each year has, and filters by one', async () => {
			const user = userEvent.setup()
			const { execute } = renderPage(
				history({
					years: [
						{ year: 2026, count: 3 },
						{ year: 2025, count: 1 },
					],
					finished: [finishedRead(dune, '2026-03-04')],
				}),
			)

			const years = await screen.findByRole('navigation', { name: 'readingHistory.finished.years' })
			expect(
				within(years)
					.getAllByRole('button')
					.map((chip) => chip.textContent),
			).toEqual(['readingHistory.finished.allYears', '20263', '20251'])
			expect(
				within(years).getByRole('button', { name: 'readingHistory.finished.allYears' }),
			).toHaveAttribute('aria-pressed', 'true')

			await user.click(within(years).getByRole('button', { name: /2025/ }))

			expect(screen.getByTestId('location')).toHaveTextContent('/reading?year=2025')
			await waitFor(() =>
				expect(sent(execute, 'ReadingHistoryFinished').at(-1)).toMatchObject({
					year: 2025,
					page: 1,
				}),
			)
			expect(within(years).getByRole('button', { name: /2025/ })).toHaveAttribute(
				'aria-pressed',
				'true',
			)

			// "All" takes the year off again
			await user.click(
				within(years).getByRole('button', { name: 'readingHistory.finished.allYears' }),
			)
			expect(screen.getByTestId('location')).toHaveTextContent(/^\/reading$/)
		})

		it('keeps the search, the year and the page in the address, and asks for them', async () => {
			const user = userEvent.setup()
			const { execute } = renderPage(
				history({
					years: [{ year: 2025, count: 90 }],
					finished: [finishedRead(dune, '2025-03-04')],
					totalPages: 3,
					currentPage: 2,
				}),
				'/reading?search=dune&year=2025&page=2',
			)

			await finishedSection()
			expect(sent(execute, 'ReadingHistoryFinished')[0]).toEqual({
				userId: undefined,
				search: 'dune',
				year: 2025,
				page: 2,
				pageSize: FINISHED_PAGE_SIZE,
			})
			// what was in the address is in the search box
			expect(screen.getByRole('textbox', { name: 'readingHistory.finished.search' })).toHaveValue(
				'dune',
			)

			// the box tells its value once as it mounts: that is the address's own search, so the page
			// the address names stays (it was lost, and the first page loaded after a reload or a Back)
			await settle()
			expect(screen.getByTestId('location')).toHaveTextContent(
				/^\/reading\?search=dune&year=2025&page=2$/,
			)
			await waitFor(() =>
				expect(sent(execute, 'ReadingHistoryFinished').at(-1)).toMatchObject({
					search: 'dune',
					year: 2025,
					page: 2,
				}),
			)

			// another page: the same search and year, a new page
			await user.click(await screen.findByText('3'))
			await waitFor(() =>
				expect(screen.getByTestId('location')).toHaveTextContent(
					'/reading?search=dune&year=2025&page=3',
				),
			)

			// a new search starts again from the first page
			const box = screen.getByRole('textbox', { name: 'readingHistory.finished.search' })
			await user.clear(box)
			await user.type(box, 'emma')
			await waitFor(() =>
				expect(screen.getByTestId('location')).toHaveTextContent(/\/reading\?.*search=emma/),
			)
			expect(screen.getByTestId('location')).not.toHaveTextContent('page=')
			expect(screen.getByTestId('location')).toHaveTextContent('year=2025')
			await waitFor(() =>
				expect(sent(execute, 'ReadingHistoryFinished').at(-1)).toMatchObject({
					search: 'emma',
					page: 1,
				}),
			)
		})

		it('keeps the page of an address that has no search, when the box tells its first value', async () => {
			const { execute } = renderPage(
				history({ years: [{ year: 2025, count: 90 }], totalPages: 5, currentPage: 3 }),
				'/reading?year=2025&page=3',
			)

			await finishedSection()
			await settle()
			expect(screen.getByTestId('location')).toHaveTextContent(/^\/reading\?year=2025&page=3$/)
			await waitFor(() =>
				expect(sent(execute, 'ReadingHistoryFinished').at(-1)).toMatchObject({
					year: 2025,
					page: 3,
				}),
			)
		})

		it('keeps the page when the search in the address has spaces around it', async () => {
			renderPage(history({ totalPages: 4, currentPage: 2 }), '/reading?search=%20dune%20&page=2')

			await finishedSection()
			await settle()
			// the box reports the same search, trimmed: nothing changes, so the address stays as it was
			expect(screen.getByTestId('location')).toHaveTextContent(
				/^\/reading\?search=%20dune%20&page=2$/,
			)
		})

		it('treats an address that is not a year or a page as no year and the first page', async () => {
			const { execute } = renderPage(
				history({ years: [{ year: 2025, count: 1 }] }),
				'/reading?year=abc&page=-4',
			)

			await finishedSection()
			expect(sent(execute, 'ReadingHistoryFinished')[0]).toMatchObject({
				year: undefined,
				page: 1,
			})
		})

		it('says nothing is finished yet, and when a search or a year finds nothing', async () => {
			renderPage(history())
			expect(await screen.findByText('readingHistory.finished.empty')).toBeInTheDocument()
		})

		it('says when a search finds nothing', async () => {
			renderPage(history(), '/reading?search=zzz')
			expect(await screen.findByText('readingHistory.finished.noMatch')).toBeInTheDocument()
			expect(screen.queryByText('readingHistory.finished.empty')).not.toBeInTheDocument()
		})

		it('has no friends to switch to when nobody shares', async () => {
			renderPage(history())
			await finishedSection()
			expect(
				screen.queryByRole('navigation', { name: 'readingHistory.people.label' }),
			).not.toBeInTheDocument()
		})

		it('lets the person switch to the friends who share, and shows who it is', async () => {
			renderPage(history({ friends: [ana] }))

			const people = await screen.findByRole('navigation', { name: 'readingHistory.people.label' })
			const links = within(people).getAllByRole('link')
			expect(links.map((link) => link.getAttribute('href'))).toEqual(['/reading', '/reading/ana'])
			expect(links[0]).toHaveAttribute('aria-current', 'page')
			expect(links[1]).not.toHaveAttribute('aria-current')
			expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent('readingHistory.title')
		})
	})

	describe('who sees the page, on a own page', () => {
		const chipOf = (name: string) => screen.findByRole('button', { name })
		const switchOf = () => screen.findByRole('switch', { name: 'readingHistory.privacy.switch' })

		it('is a small chip that says the page is private until the person turns sharing on', async () => {
			renderPage(history())

			expect(await chipOf('readingHistory.privacy.privateLabel')).toHaveTextContent(
				'readingHistory.privacy.private',
			)
			// what it shares is there when asked for, not a block of the page
			expect(screen.queryByText('readingHistory.privacy.description')).not.toBeInTheDocument()
			expect(screen.queryByRole('switch')).not.toBeInTheDocument()
		})

		it('puts the reading first: the chip is with the title, and the page starts with Reading now', async () => {
			renderPage(
				history({
					reading: [readingNow(ulysses, 12, '2026-03-02')],
					years: [{ year: 2026, count: 1 }],
					finished: [finishedRead(dune, '2026-03-04')],
				}),
			)

			const reading = await screen.findByText('Ulysses')
			await screen.findByText('Dune')
			const chip = await chipOf('readingHistory.privacy.privateLabel')

			// nothing but the reading is in the page: its first region is Reading now
			expect(screen.getAllByRole('region')[0]).toHaveAccessibleName('readingHistory.now.heading')
			expect(screen.queryByRole('region', { name: 'readingHistory.privacy.label' })).toBeNull()

			// the chip is in the header, with the title (a chip back in the page body would fail here:
			// the title bar is the only banner on the page)
			const banner = screen.getByRole('banner')
			expect(
				within(banner).getByRole('button', { name: 'readingHistory.privacy.privateLabel' }),
			).toBe(chip)
			expect(within(banner).getByRole('heading', { level: 1 })).toBeInTheDocument()
			// and the header comes before the reading
			expect(
				banner.compareDocumentPosition(reading) & Node.DOCUMENT_POSITION_FOLLOWING,
			).toBeTruthy()
		})

		it('opens to the switch, with what it shares told in the words', async () => {
			const user = userEvent.setup()
			renderPage(history())

			await user.click(await chipOf('readingHistory.privacy.privateLabel'))

			expect(await switchOf()).toHaveAttribute('aria-checked', 'false')
			expect(screen.getByText('readingHistory.privacy.description')).toBeInTheDocument()
		})

		it('lets everyone see it when turned on, and the chip says so', async () => {
			const user = userEvent.setup()
			const { execute } = renderPage(history())

			await user.click(await chipOf('readingHistory.privacy.privateLabel'))
			await user.click(await switchOf())

			await waitFor(() =>
				expect(sent(execute, 'UpdateMyReadingPrivacy')).toEqual([{ share: true }]),
			)
			expect(await chipOf('readingHistory.privacy.sharedLabel')).toHaveTextContent(
				'readingHistory.privacy.shared',
			)
			expect(await switchOf()).toHaveAttribute('aria-checked', 'true')
			// the page loads again with what everyone may see now
			await waitFor(() => expect(sent(execute, 'ReadingHistoryPeople').length).toBeGreaterThan(1))
		})

		it('stops it when turned off again', async () => {
			const user = userEvent.setup()
			const { execute } = renderPage(history({ sharing: true }))

			await user.click(await chipOf('readingHistory.privacy.sharedLabel'))
			await user.click(await switchOf())

			await waitFor(() =>
				expect(sent(execute, 'UpdateMyReadingPrivacy')).toEqual([{ share: false }]),
			)
			expect(await chipOf('readingHistory.privacy.privateLabel')).toBeInTheDocument()
		})
	})

	describe("a friend's page", () => {
		const friendsReading = () =>
			history({
				isSelf: false,
				friends: [ana],
				reading: [readingNow(ulysses, 12, '2026-03-02')],
				years: [{ year: 2026, count: 1 }],
				finished: [finishedRead(dune, '2026-03-04')],
			})

		it("shows the friend's reading, asked for by the friend's id", async () => {
			const { execute } = renderPage(friendsReading(), '/reading/ana')

			expect(await screen.findByText('Ulysses')).toBeInTheDocument()
			expect(await screen.findByText('Dune')).toBeInTheDocument()
			expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent(
				'readingHistory.titleOf name=ana',
			)
			for (const name of ['ReadingHistoryReader', 'ReadingHistoryNow', 'ReadingHistoryFinished']) {
				expect(sent(execute, name)[0]).toMatchObject({ userId: 'ana' })
			}

			// the person asking is on the list as themselves, and the friend is where they are
			const people = screen.getByRole('navigation', { name: 'readingHistory.people.label' })
			const links = within(people).getAllByRole('link')
			expect(links[0]).not.toHaveAttribute('aria-current')
			expect(links[1]).toHaveAttribute('aria-current', 'page')
		})

		it('is read-only: no chip or switch to change who sees it, and the title says whose page it is', async () => {
			renderPage(friendsReading(), '/reading/ana')

			await screen.findByText('Dune')
			expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent(
				'readingHistory.titleOf name=ana',
			)
			expect(screen.queryByRole('switch')).not.toBeInTheDocument()
			expect(
				screen.queryByRole('button', { name: /readingHistory\.privacy\./ }),
			).not.toBeInTheDocument()
			expect(screen.queryByRole('button', { name: /remove|delete/i })).not.toBeInTheDocument()
		})

		it('is not available when the friend does not share, or does not exist: the same page for both', async () => {
			const { execute } = renderPage(
				history({ isSelf: false, friends: [ana], refuse: true }),
				'/reading/nobody',
			)

			expect(await screen.findByText('readingHistory.unavailable.title')).toBeInTheDocument()
			expect(screen.getByText('readingHistory.unavailable.description')).toBeInTheDocument()
			// nothing of a reading is asked for, shown or changeable
			expect(sent(execute, 'ReadingHistoryNow')).toEqual([])
			expect(sent(execute, 'ReadingHistoryFinished')).toEqual([])
			expect(screen.queryByRole('switch')).not.toBeInTheDocument()
			// and the person can still go back to their own reading and to the friends who share
			const people = await screen.findByRole('navigation', { name: 'readingHistory.people.label' })
			expect(
				within(people)
					.getAllByRole('link')
					.map((link) => link.getAttribute('href')),
			).toEqual(['/reading', '/reading/ana'])
		})
	})
})
