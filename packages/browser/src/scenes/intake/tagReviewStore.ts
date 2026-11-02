import { create } from 'zustand'
import { createJSONStorage, persist } from 'zustand/middleware'

type TagReviewStore = {
	/** Inbox items whose tags the uploader has touched (their tags are then taken as is) */
	reviewed: Record<string, true>
	markReviewed: (id: string) => void
	/** Forget items no longer in the inbox */
	keepOnly: (ids: string[]) => void
}

/**
 * Remembers, per inbox item and in this browser, that the uploader chose its tags. Until
 * then weak guesses stay unticked (see `effectiveGenres`); afterwards the server's tags are
 * exactly the uploader's choice, even if that matches the server's first proposal.
 */
export const useTagReviewStore = create<TagReviewStore>()(
	persist(
		(set, get) => ({
			keepOnly: (ids) => {
				const keep = new Set(ids)
				const current = get().reviewed
				if (Object.keys(current).every((id) => keep.has(id))) return
				set({
					reviewed: Object.fromEntries(
						Object.keys(current)
							.filter((id) => keep.has(id))
							.map((id) => [id, true as const]),
					),
				})
			},
			markReviewed: (id) => {
				if (!get().reviewed[id]) set({ reviewed: { ...get().reviewed, [id]: true } })
			},
			reviewed: {},
		}),
		{
			name: 'stump-intake-tag-review',
			partialize: ({ reviewed }) => ({ reviewed }),
			storage: createJSONStorage(() => localStorage),
		},
	),
)
