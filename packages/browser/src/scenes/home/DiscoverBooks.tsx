import { PREFETCH_STALE_TIME, useGraphQL, useGraphQLMutation, useSDK } from '@stump/client'
import { Button, cn, Text, ToolTip } from '@stump/components'
import {
	DiscoverBooksQuery,
	FragmentType,
	graphql,
	useFragment,
	UserPermission,
} from '@stump/graphql'
import { Api } from '@stump/sdk'
import { QueryClient, useQueryClient } from '@tanstack/react-query'
import { Dices, RefreshCw, Sparkles, X } from 'lucide-react'
import { memo, useCallback, useMemo, useState } from 'react'
import { useMediaMatch } from 'rooks'
import { toast } from 'sonner'

import MultiRowHorizontalCardList from '@/components/MultiRowHorizontalCardList'
import { ThumbnailImage } from '@/components/thumbnail/ThumbnailImage'
import { ThumbnailPlaceholderData } from '@/components/thumbnail/ThumbnailPlaceholder'
import { Link, useAppContext } from '@/context'
import { usePreferences } from '@/hooks/usePreferences'
import { UNREAD_FILTER, useSurpriseMe } from '@/hooks/useSurpriseMe'
import { useTranslate } from '@/hooks/useTranslate'
import { usePaths } from '@/paths'

const IMAGE_WIDTH_MOBILE = 120
const IMAGE_WIDTH_TABLET = 150
const PICKS = 16

const DiscoverBookFragment = graphql(`
	fragment DiscoverBook on Media {
		id
		resolvedName
		tags {
			name
		}
		metadata {
			writers
			rating
		}
		thumbnail {
			url
			metadata {
				averageColor
				colors {
					color
					percentage
				}
				thumbhash
			}
		}
	}
`)

const vetoMutation = graphql(`
	mutation DiscoverBooksVeto($id: ID!, $tags: [String!]!) {
		setMediaTags(id: $id, tags: $tags) {
			id
		}
	}
`)

/** The tag that keeps a book (or a series or library) out of the picks; see `randomMedia` */
export const NO_PICKS_TAG = 'No picks'

const query = graphql(`
	query DiscoverBooks($filter: MediaFilterInput!, $count: Int!, $seed: String) {
		randomMedia(filter: $filter, count: $count, seed: $seed) {
			id
			...DiscoverBook
		}
	}
`)

/** The ISO week, e.g. `2026-W45`: the picks change on Mondays */
export function isoWeek(date = new Date()) {
	const day = new Date(Date.UTC(date.getFullYear(), date.getMonth(), date.getDate()))
	const weekday = day.getUTCDay() || 7
	day.setUTCDate(day.getUTCDate() + 4 - weekday)
	const yearStart = Date.UTC(day.getUTCFullYear(), 0, 1)
	const week = Math.ceil(((day.getTime() - yearStart) / 86_400_000 + 1) / 7)
	return `${day.getUTCFullYear()}-W${String(week).padStart(2, '0')}`
}

const SHUFFLE_KEY = 'stump.discover.shuffle'

/** How many times the week's picks were shuffled, kept for the week in this browser */
function readShuffle(week: string) {
	try {
		const saved = JSON.parse(localStorage.getItem(SHUFFLE_KEY) ?? 'null')
		return saved?.week === week && Number.isInteger(saved.count) ? (saved.count as number) : 0
	} catch {
		return 0
	}
}

function writeShuffle(week: string, count: number) {
	try {
		localStorage.setItem(SHUFFLE_KEY, JSON.stringify({ week, count }))
	} catch {
		// private mode: the shuffle lasts until the page is left
	}
}

const seedFor = (week: string, shuffle: number) => (shuffle ? `${week}#${shuffle}` : week)
const queryKey = (sdk: Api, seed: string) => sdk.cacheKey('randomMedia', [seed])
const variables = (seed: string) => ({ filter: UNREAD_FILTER, count: PICKS, seed })

/** Plain version for callers outside React (the sidebar loads it on demand) */
export const prefetchDiscoverBooks = (client: QueryClient, sdk: Api) => {
	const week = isoWeek()
	const seed = seedFor(week, readShuffle(week))
	return client.prefetchQuery({
		queryKey: queryKey(sdk, seed),
		queryFn: () => sdk.execute(query, variables(seed)),
		staleTime: PREFETCH_STALE_TIME,
	})
}

export const usePrefetchDiscoverBooks = () => {
	const { sdk } = useSDK()
	const client = useQueryClient()
	return useCallback(() => prefetchDiscoverBooks(client, sdk), [sdk, client])
}

/**
 * "Picks of the week": a row of unread books picked at random, the same all week (the server
 * seeds the pick with the week and the user), so it reads like a feed rather than noise.
 * Shuffle picks another set for the rest of the week; the dice open one random unread book.
 */
export default function DiscoverBooks() {
	const { translate } = useTranslate()
	const { sdk } = useSDK()
	const {
		preferences: { thumbnailRatio },
	} = usePreferences()
	const isAtLeastMedium = useMediaMatch('(min-width: 768px)')
	const cardWidth = isAtLeastMedium ? IMAGE_WIDTH_TABLET : IMAGE_WIDTH_MOBILE
	const cardHeight = cardWidth / thumbnailRatio

	const week = useMemo(() => isoWeek(), [])
	const [shuffle, setShuffle] = useState(() => readShuffle(week))
	const seed = seedFor(week, shuffle)

	const { data, isFetching, isPending } = useGraphQL(
		query,
		queryKey(sdk, seed),
		variables(seed),
		// the old picks stay on screen while a shuffle loads
		{ placeholderData: (previous) => previous, staleTime: 60 * 60 * 1000 },
	)
	const nodes = data?.randomMedia ?? []

	const reshuffle = () => {
		const next = shuffle + 1
		writeShuffle(week, next)
		setShuffle(next)
	}

	const { surprise, isPending: isSurprising } = useSurpriseMe()

	// "Not for picks": tags the book `No picks` (kept with its other tags), drops it from the
	// row at once, and offers Undo
	const client = useQueryClient()
	const { checkPermission } = useAppContext()
	const canVeto = checkPermission(UserPermission.EditMetadata)
	const { mutateAsync: setTags } = useGraphQLMutation(vetoMutation)
	const veto = useCallback(
		async (id: string, name: string, tags: string[]) => {
			const key = queryKey(sdk, seed)
			const previous = client.getQueryData<DiscoverBooksQuery>(key)
			client.setQueryData<DiscoverBooksQuery>(key, (data) =>
				data ? { randomMedia: data.randomMedia.filter((book) => book.id !== id) } : data,
			)
			try {
				await setTags({ id, tags: [...tags, NO_PICKS_TAG] })
			} catch {
				client.setQueryData(key, previous)
				toast.error("Couldn't change the book's tags")
				return
			}
			toast(`“${name}” won't be picked again`, {
				description: `Tagged ${NO_PICKS_TAG}; remove the tag in the book's settings to undo later`,
				action: {
					label: 'Undo',
					onClick: async () => {
						await setTags({ id, tags })
						client.setQueryData(key, previous)
					},
				},
			})
		},
		[client, setTags, sdk, seed],
	)

	if (isPending) {
		// keeps the row's room so the sections below don't jump
		return <div style={{ height: cardHeight + 40 }} aria-hidden />
	}

	const label = translate('shared.homeSections.discover.label')

	const actions = (
		<>
			<ToolTip content={translate('shared.homeSections.discover.shuffle')} align="end">
				<Button
					variant="ghost"
					size="icon"
					onClick={reshuffle}
					disabled={isFetching}
					aria-label={translate('shared.homeSections.discover.shuffle')}
				>
					<RefreshCw className={isFetching ? 'h-4 w-4 animate-spin' : 'h-4 w-4'} />
				</Button>
			</ToolTip>
			<ToolTip content={translate('shared.homeSections.discover.surprise')} align="end">
				<Button
					variant="ghost"
					size="icon"
					onClick={() => surprise()}
					disabled={isSurprising}
					aria-label={translate('shared.homeSections.discover.surprise')}
				>
					<Dices className="h-4 w-4" />
				</Button>
			</ToolTip>
		</>
	)

	const emptyState = (
		<div className="space-x-3 px-4 py-4 flex items-start justify-start rounded-lg border border-dashed border-border">
			<span className="p-2 rounded-lg border border-border bg-muted">
				<Sparkles className="h-8 w-8 text-muted-foreground" />
			</span>
			<div>
				<Text>{translate('shared.homeSections.discover.emptyText')}</Text>
				<Text size="sm" variant="muted">
					{translate('shared.homeSections.discover.emptyMessage')}
				</Text>
			</div>
		</div>
	)

	return (
		<MultiRowHorizontalCardList
			title={label}
			actions={actions}
			restoreKey={`home-discover-${seed}`}
			items={nodes}
			keyExtractor={(node) => node.id}
			renderItem={(node) => (
				<DiscoverBookCard
					fragment={node}
					cardWidth={cardWidth}
					onVeto={canVeto ? veto : undefined}
				/>
			)}
			cardHeight={cardHeight}
			rowCount={1}
			emptyState={emptyState}
		/>
	)
}

type CardProps = {
	fragment: FragmentType<typeof DiscoverBookFragment>
	cardWidth: number
	/** Keeps the book out of the picks (only for users who can edit tags) */
	onVeto?: (id: string, name: string, tags: string[]) => void
}

const gradient = {
	colors: ['transparent', 'transparent', 'rgba(0, 0, 0, 0.4)', 'rgba(0, 0, 0, 0.85)'],
	direction: 'to bottom',
}
const textShadow = { textShadow: '1px 1px 2px rgba(0, 0, 0, 0.5)' }

const DiscoverBookCard = memo(function DiscoverBookCard({
	fragment,
	cardWidth,
	onVeto,
}: CardProps) {
	const data = useFragment(DiscoverBookFragment, fragment)
	const paths = usePaths()
	const {
		preferences: { thumbnailRatio },
	} = usePreferences()

	const placeholderData: ThumbnailPlaceholderData | undefined = useMemo(() => {
		const meta = data.thumbnail.metadata
		if (!meta) return undefined
		return { averageColor: meta.averageColor, colors: meta.colors, thumbhash: meta.thumbhash }
	}, [data.thumbnail.metadata])

	const rating = data.metadata?.rating != null ? Number(data.metadata.rating) : null
	const subtitle = [
		data.metadata?.writers?.[0],
		rating && rating > 0 ? `★ ${rating.toFixed(1)}` : null,
	]
		.filter(Boolean)
		.join(' · ')

	return (
		<div className="group relative shrink-0" style={{ width: cardWidth }}>
			<Link
				to={paths.bookOverview(data.id)}
				className="relative block rounded-thumbnail transition-opacity hover:opacity-90"
				title={data.resolvedName}
			>
				<ThumbnailImage
					src={data.thumbnail.url}
					alt={data.resolvedName}
					size={{ width: cardWidth, height: cardWidth / thumbnailRatio }}
					placeholderData={placeholderData}
					gradient={gradient}
					borderAndShadowStyle={{ shadowColor: 'rgba(0, 0, 0, 0.2)', shadowRadius: 2 }}
				/>
				<div className="bottom-0 left-0 right-0 p-2 pointer-events-none absolute z-30">
					<Text
						className="text-sm font-semibold leading-tight text-white line-clamp-2 text-wrap!"
						style={textShadow}
					>
						{data.resolvedName}
					</Text>
					{subtitle && (
						<Text className="mt-0.5 text-xs line-clamp-1 text-gray-200" style={textShadow}>
							{subtitle}
						</Text>
					)}
				</div>
			</Link>
			{onVeto && (
				<button
					type="button"
					onClick={() =>
						onVeto(
							data.id,
							data.resolvedName,
							data.tags.map((tag) => tag.name),
						)
					}
					aria-label={`Not for picks: ${data.resolvedName}`}
					title="Not for picks"
					className={cn(
						'right-1.5 top-1.5 size-7 bg-black/60 text-white hover:bg-black/80 absolute z-40 flex items-center justify-center rounded-full transition-opacity',
						'opacity-0 group-focus-within:opacity-100 group-hover:opacity-100 focus-visible:opacity-100 pointer-coarse:opacity-100',
					)}
				>
					<X className="h-4 w-4" />
				</button>
			)}
		</div>
	)
})
