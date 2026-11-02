import { APIBase } from '../base'
import type {
	PdfLayout,
	PdfPageText,
	PdfPageURLParams,
	PdfSearchParams,
	PdfSearchResponse,
} from '../types/pdf'
import { ClassQueryKeys } from './types'
import { createRouteURLHandler } from './utils'

/**
 * The root route for the PDF reader API
 */
const PDF_ROUTE = '/pdf'
const pdfURL = createRouteURLHandler(PDF_ROUTE)

/**
 * The PDF reader API: the document layout, page pictures sized for the screen, each page's
 * text layer and a whole-book search. It needs the same access as reading the book, not the
 * download permission.
 */
export class PdfAPI extends APIBase {
	/**
	 * Absolute URL of a page picture (1-based `page`)
	 */
	pageURL(id: string, page: number, { w, v, prefetch }: PdfPageURLParams): string {
		return this.withServiceURL(
			pdfURL(`/${id}/page/${page}`, {
				w: Math.max(1, Math.round(w)),
				...(v ? { v } : {}),
				...(prefetch ? { p: 'prefetch' } : {}),
			}),
		)
	}

	/**
	 * The page sizes, outline, labels and text availability of a PDF. With `firstPage`, the
	 * server starts rendering that page's picture (1-based `page`, width `w`) right behind
	 * the layout, so the page request that follows finds it in flight or cached
	 */
	async layout(
		id: string,
		signal?: AbortSignal,
		firstPage?: { page: number; w: number },
	): Promise<PdfLayout> {
		const params = firstPage
			? { page: Math.max(1, Math.round(firstPage.page)), w: Math.max(1, Math.round(firstPage.w)) }
			: undefined
		const { data } = await this.api.axios.get<PdfLayout>(pdfURL(`/${id}/layout`, params), {
			signal,
		})
		return data
	}

	/**
	 * The text layer of a page (1-based `page`). A `prefetch` is low priority and may be
	 * dropped by the server (503) when the reader has moved elsewhere
	 */
	async text(
		id: string,
		page: number,
		v?: string,
		signal?: AbortSignal,
		{ prefetch }: { prefetch?: boolean } = {},
	): Promise<PdfPageText> {
		const { data } = await this.api.axios.get<PdfPageText>(
			pdfURL(`/${id}/text/${page}`, {
				...(v ? { v } : {}),
				...(prefetch ? { p: 'prefetch' } : {}),
			}),
			{ signal },
		)
		return data
	}

	/**
	 * Whole-book search; continue with `nextCursor` until it is `null`
	 */
	async search({ id, q, limit, cursor, signal }: PdfSearchParams): Promise<PdfSearchResponse> {
		const { data } = await this.api.axios.get<PdfSearchResponse>(
			pdfURL(`/${id}/search`, {
				q,
				...(limit != null ? { limit } : {}),
				...(cursor ? { cursor } : {}),
			}),
			{ signal },
		)
		return data
	}

	/**
	 * The query keys for the PDF API, used for query caching on a client (e.g. react-query)
	 */
	get keys(): ClassQueryKeys<InstanceType<typeof PdfAPI>> {
		return {
			layout: 'pdf.layout',
			pageURL: 'pdf.pageURL',
			search: 'pdf.search',
			text: 'pdf.text',
		}
	}
}
