import { useSDK } from '@stump/client'
import { cn } from '@stump/components'
import { useMemo, useState } from 'react'

import { AuthImage } from '@/components/entity/AuthImage'

import { FormatIcon } from './FormatIcon'

/** A tile's size: `md` for a card, `sm` for a row */
type TileSize = 'sm' | 'md'

const TILE_SIZE: Record<TileSize, string> = {
	md: 'h-[4.5rem] w-12',
	sm: 'h-[3.25rem] w-9',
}

/** The first letters of the first two words of a title, as a monogram: `El nombre de la rosa` → `EN` */
export function monogramOf(title: string): string {
	const words = title.match(/[\p{L}\p{N}]+/gu) ?? []
	return words
		.slice(0, 2)
		.map((word) => word.charAt(0))
		.join('')
		.toUpperCase()
}

/** A hue from the title, so the same book always gets the same tile (0–359) */
export function hueOf(title: string): number {
	let hash = 0
	for (const char of title) hash = (hash * 31 + (char.codePointAt(0) ?? 0)) >>> 0
	return hash % 360
}

type TileProps = {
	title: string
	format?: string
	/** A series of volumes rather than a single book */
	series?: boolean
	size?: TileSize
	className?: string
}

/**
 * A book's cover before it has one: a tinted tile with a spine, the format's icon and (on the
 * larger size) the title's monogram, like the front of a paperback. Its colour comes from the
 * title and is translucent, so it sits well on a light or a dark card.
 */
export default function CoverTile({ title, format, series, size = 'md', className }: TileProps) {
	const hue = useMemo(() => hueOf(title), [title])
	const large = size === 'md'
	return (
		<div
			aria-hidden
			className={cn(
				'relative flex shrink-0 overflow-hidden rounded-md border',
				TILE_SIZE[size],
				className,
			)}
			style={{
				backgroundColor: `hsl(${hue} 40% 50% / 0.16)`,
				borderColor: `hsl(${hue} 40% 50% / 0.35)`,
			}}
		>
			<span
				className="inset-y-0 left-0 w-1 absolute"
				style={{ backgroundColor: `hsl(${hue} 45% 50% / 0.55)` }}
			/>
			<div
				className={cn(
					'pr-1 pl-2 flex flex-1 flex-col',
					large ? 'py-1.5 justify-between' : 'items-center justify-center',
				)}
			>
				<FormatIcon
					format={format}
					series={series}
					className={cn('text-foreground/60', large ? 'h-3.5 w-3.5' : 'h-4 w-4')}
				/>
				{large && (
					<span className="text-lg font-bold tracking-tight leading-none text-foreground/80">
						{monogramOf(title)}
					</span>
				)}
			</div>
		</div>
	)
}

type BookCoverProps = TileProps & {
	/** The book the scan made of it */
	mediaId: string
}

/**
 * The cover of a book that is in the library: its thumbnail, fading in over the tile, which stays
 * where the picture is not there yet and in its place when it can't be had
 */
export function BookCover({ mediaId, size = 'sm', ...tile }: BookCoverProps) {
	const { sdk } = useSDK()
	const [loaded, setLoaded] = useState(false)
	const [failed, setFailed] = useState(false)
	const imageProps = {
		alt: '',
		className: cn(
			'inset-0 absolute h-full w-full rounded-md object-cover transition-opacity',
			loaded ? 'opacity-100' : 'opacity-0',
		),
		decoding: 'async' as const,
		loading: 'lazy' as const,
		onError: () => setFailed(true),
		onLoad: () => setLoaded(true),
		src: sdk.media.thumbnailURL(mediaId),
	}
	return (
		<div className="relative shrink-0">
			<CoverTile size={size} {...tile} />
			{!failed &&
				(sdk.isTokenAuth ? (
					// the server wants the token with the request, which an <img> can't send
					<AuthImage token={sdk.token || ''} {...imageProps} />
				) : (
					<img {...imageProps} />
				))}
		</div>
	)
}
