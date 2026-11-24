import { APIBase } from '../base'
import type { ContentSearchParams, ContentSearchResponse, ContentStatus } from '../types/content'
import { ClassQueryKeys } from './types'
import { createRouteURLHandler } from './utils'

const CONTENT_ROUTE = '/content'
const contentURL = createRouteURLHandler(CONTENT_ROUTE)

/**
 * The search inside the books' text. The server shows only the books the user may see.
 */
export class ContentAPI extends APIBase {
	/**
	 * The passages that match `q`, best first, the best of each book
	 */
	async search({
		q,
		match,
		page,
		page_size,
		library_id,
		media_id,
		signal,
	}: ContentSearchParams): Promise<ContentSearchResponse> {
		const { data } = await this.api.axios.get<ContentSearchResponse>(
			contentURL('/search', {
				q,
				...(match ? { match } : {}),
				...(page ? { page } : {}),
				...(page_size ? { page_size } : {}),
				...(library_id ? { library_id } : {}),
				...(media_id ? { media_id } : {}),
			}),
			{ signal },
		)
		return data
	}

	/** How many of the books are in the text index */
	async status(signal?: AbortSignal): Promise<ContentStatus> {
		const { data } = await this.api.axios.get<ContentStatus>(contentURL('/status'), { signal })
		return data
	}

	get keys(): ClassQueryKeys<InstanceType<typeof ContentAPI>> {
		return {
			search: 'content.search',
			status: 'content.status',
		}
	}
}
