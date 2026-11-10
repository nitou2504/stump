import '@/__mocks__/resizeObserver'

import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { Route, Routes } from 'react-router'

import { FINISHED_PAGE_SIZE } from '../graphql'
import ReadingHistoryScene from '../index'
import { ana, finishedRead, history, readingNow, sent, setup } from './support'

vi.mock('@/hooks/useTranslate', () => import('./translations'))
vi.mock('../availability', () => ({ MY_READING_AVAILABLE: true }))
// the header, the covers and the search are tested where they are; here what the page puts in them
vi.mock('@/components/sharedLayout', () => ({
	CollectionSceneHeader: ({ title }: { title: string }) => <h1>{title}</h1>,
}))
vi.mock('@/components/thumbnail', () => ({
	ThumbnailImage: ({ src, alt }: { src: string; alt?: string }) => <img src={src} alt={alt} />,
}))
vi.mock('@/components/filters', () => ({
	Search: ({
		initialValue,
		placeholder,
		onChange,
	}: {
		initialValue?: string
		placeholder?: string
		onChange: (value: string) => void
	}) => (
		<input
			aria-label={placeholder}
			defaultValue={initialValue}
			onChange={(event) => onChange(event.target.value)}
		/>
	),
}))

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

			// the day it was read last, on the calendar of the reader, and which read it is
			expect(
				within(now).getByText(/readingHistory\.now\.lastRead date=Mar 9, 2026/),
			).toHaveTextContent('readingHistory.read count=2 ordinal=true')
			expect(within(now).getByText(/date=Mar 1, 2026/)).not.toHaveTextContent('ordinal')
		})

		it('says there is nothing in progress', async () => {
			renderPage(history())
			expect(await screen.findByText('readingHistory.now.empty')).toBeInTheDocument()
		})

		it('lists the finished books in months, newest first, with their dates and which read they were', async () => {
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
			expect(emmaRow).toHaveTextContent('readingHistory.read count=2 ordinal=true')
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

	describe('the privacy switch on a own page', () => {
		const switchOf = () => screen.findByRole('switch', { name: 'readingHistory.privacy.switch' })

		it('says that only the person sees the page until they turn it on', async () => {
			renderPage(history())

			expect(await screen.findByText('readingHistory.privacy.privateTitle')).toBeInTheDocument()
			expect(await switchOf()).toHaveAttribute('aria-checked', 'false')
		})

		it('lets everyone see it when turned on, and says so', async () => {
			const user = userEvent.setup()
			const { execute } = renderPage(history())

			await user.click(await switchOf())

			await waitFor(() =>
				expect(sent(execute, 'UpdateMyReadingPrivacy')).toEqual([{ share: true }]),
			)
			expect(await screen.findByText('readingHistory.privacy.sharedTitle')).toBeInTheDocument()
			expect(await switchOf()).toHaveAttribute('aria-checked', 'true')
			// the page loads again with what everyone may see now
			await waitFor(() => expect(sent(execute, 'ReadingHistoryPeople').length).toBeGreaterThan(1))
		})

		it('stops it when turned off again', async () => {
			const user = userEvent.setup()
			const { execute } = renderPage(history({ sharing: true }))

			expect(await screen.findByText('readingHistory.privacy.sharedTitle')).toBeInTheDocument()
			await user.click(await switchOf())

			await waitFor(() =>
				expect(sent(execute, 'UpdateMyReadingPrivacy')).toEqual([{ share: false }]),
			)
			expect(await screen.findByText('readingHistory.privacy.privateTitle')).toBeInTheDocument()
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

		it('is read-only: no switch to change who sees it, and nothing to remove', async () => {
			renderPage(friendsReading(), '/reading/ana')

			await screen.findByText('Dune')
			expect(screen.queryByRole('switch')).not.toBeInTheDocument()
			expect(screen.queryByText('readingHistory.privacy.privateTitle')).not.toBeInTheDocument()
			expect(screen.queryByText('readingHistory.privacy.sharedTitle')).not.toBeInTheDocument()
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
