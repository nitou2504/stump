import { UserPermission } from '@stump/graphql'
import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router'

import { AppContext, IAppContext } from '@/context'

import { PdfOpenAs } from '../PdfSettings'

vi.mock('@/hooks/useTranslate', () => ({
	useTranslate: () => ({ t: (key: string) => key, translate: (key: string) => key }),
}))

const renderLinks = (permissions: UserPermission[]) => {
	const app = {
		checkPermission: (permission: UserPermission) => permissions.includes(permission),
	} as unknown as IAppContext

	return render(
		<AppContext.Provider value={app}>
			<MemoryRouter>
				<PdfOpenAs bookId="book" onChange={vi.fn()} />
			</MemoryRouter>
		</AppContext.Provider>,
	)
}

describe('PdfOpenAs', () => {
	it("offers the pages view to everyone, and the browser's own viewer to who may download", () => {
		renderLinks([UserPermission.DownloadFile])

		expect(screen.getByRole('button', { name: 'shared.pdfReader.openAsPages' })).toBeInTheDocument()
		expect(screen.getByRole('link', { name: 'shared.pdfReader.openInBrowser' })).toBeInTheDocument()
	})

	it("hides the browser's own viewer from who may not: it fetches the original file", () => {
		renderLinks([])

		expect(screen.getByRole('button', { name: 'shared.pdfReader.openAsPages' })).toBeInTheDocument()
		expect(
			screen.queryByRole('link', { name: 'shared.pdfReader.openInBrowser' }),
		).not.toBeInTheDocument()
	})
})
