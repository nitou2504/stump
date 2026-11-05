import { useGraphQL } from '@stump/client'
import { Button, cn } from '@stump/components'
import { IntakeSource, MediaDescriptionStatus } from '@stump/graphql'
import { Globe, X } from 'lucide-react'
import { useState } from 'react'

import { lookupLine, lookupNeedsAttention, sourceName } from './onlineLookups'
import { mediaDescriptionSuggestionsQuery } from './queries'

type Props = {
	mediaId: string
	onApply: (summary: string) => void
	onClose: () => void
}

/**
 * Descriptions found online for a book (Hardcover first, then Open Library and Google Books):
 * one is shown with where it came from, and "Use this description" puts it into the summary.
 * Nothing is saved until "Save"
 */
export default function DescriptionPanel({ mediaId, onApply, onClose }: Props) {
	const { data, isFetching, isError } = useGraphQL(
		mediaDescriptionSuggestionsQuery,
		['mediaDescriptionSuggestions', mediaId],
		{ id: mediaId },
		{ staleTime: 5 * 60 * 1000 },
	)
	const result = data?.mediaDescriptionSuggestions
	const candidates = result?.candidates ?? []
	const [picked, setPicked] = useState<IntakeSource | null>(null)
	const shown = candidates.find((c) => c.source === picked) ?? candidates[0]

	return (
		<section
			aria-label="Descriptions found online"
			className="gap-3 p-3 flex flex-col rounded-lg border border-border bg-muted/30"
		>
			<header className="gap-2 flex items-center">
				<Globe className="size-4 shrink-0 text-primary" aria-hidden />
				<h3 className="text-sm font-medium flex-1">Descriptions found online</h3>
				<Button
					type="button"
					variant="ghost"
					size="icon"
					className="size-7 pointer-coarse:size-10"
					aria-label="Close the descriptions"
					onClick={onClose}
				>
					<X />
				</Button>
			</header>

			{isFetching && (
				<p className="text-xs text-muted-foreground" role="status">
					Looking online…
				</p>
			)}
			{isError && !isFetching && (
				<p className="text-xs text-destructive">Couldn’t look online. Try again.</p>
			)}

			{result && !isFetching && (
				<>
					{result.onlineLookups.length > 0 && (
						<ul className="gap-0.5 text-xs flex flex-col" aria-label="Online lookups">
							{result.onlineLookups.map((lookup) => (
								<li
									key={lookup.source}
									className={cn(
										lookupNeedsAttention(lookup.status)
											? 'text-amber-700 dark:text-amber-400'
											: 'text-muted-foreground',
									)}
								>
									{lookupLine(lookup)}
								</li>
							))}
						</ul>
					)}
					{result.searchedTitle && (
						<p className="text-xs text-muted-foreground">
							Searched as “{result.searchedTitle}”
							{result.searchedAuthors.length > 0 && ` by ${result.searchedAuthors.join(', ')}`}: the
							book’s own details looked wrong.
						</p>
					)}

					{shown ? (
						<>
							{candidates.length > 1 && (
								<div className="gap-1.5 flex flex-wrap" role="group" aria-label="Sources">
									{candidates.map((c) => (
										<Button
											key={c.source}
											type="button"
											size="xs"
											variant={c.source === shown.source ? 'secondary' : 'ghost'}
											aria-pressed={c.source === shown.source}
											onClick={() => setPicked(c.source)}
										>
											{sourceName(c.source)}
										</Button>
									))}
								</div>
							)}
							<figure className="gap-1 flex flex-col">
								<blockquote className="text-sm max-h-48 p-2 overflow-y-auto rounded-md border border-border bg-background whitespace-pre-line">
									{shown.text}
								</blockquote>
								<figcaption className="text-xs text-muted-foreground">
									{sourceName(shown.source)} · {shown.words} {shown.words === 1 ? 'word' : 'words'}
								</figcaption>
							</figure>
						</>
					) : (
						<p className="text-xs text-muted-foreground">
							{result.status === MediaDescriptionStatus.RetryLater
								? 'No description found yet. Try again later.'
								: 'No description found.'}
						</p>
					)}

					<footer className="gap-2 flex items-center justify-end">
						<Button type="button" variant="ghost" size="sm" onClick={onClose}>
							Cancel
						</Button>
						<Button
							type="button"
							size="sm"
							disabled={!shown}
							onClick={() => shown && onApply(shown.text)}
						>
							Use this description
						</Button>
					</footer>
				</>
			)}
		</section>
	)
}
