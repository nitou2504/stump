import { useSDK } from '@stump/client'
import { cn, Command } from '@stump/components'
import type { PdfSearchResult } from '@stump/sdk'
import { Loader2, Search } from 'lucide-react'
import { useCallback, useEffect, useRef, useState } from 'react'

import Spinner from '@/components/Spinner'
import { useTranslate } from '@/hooks/useTranslate'

type Props = {
	mediaId: string
	open: boolean
	onOpenChange: (open: boolean) => void
	/** Label of a 1-based page (the PDF's own label or the number) */
	labelOf: (page: number) => string
	onSelect: (result: PdfSearchResult) => void
}

/**
 * Whole-book search (accent- and case-insensitive, on the server). Results stream in pages;
 * selecting one goes to its page and flashes the hit.
 */
export default function PdfSearchDialog({ mediaId, open, onOpenChange, labelOf, onSelect }: Props) {
	const { sdk } = useSDK()
	const { translate } = useTranslate()

	const [query, setQuery] = useState('')
	const [results, setResults] = useState<PdfSearchResult[]>([])
	const [nextCursor, setNextCursor] = useState<string | null>(null)
	const [isSearching, setIsSearching] = useState(false)
	const [isLoadingMore, setIsLoadingMore] = useState(false)
	const [hasSearched, setHasSearched] = useState(false)
	const [error, setError] = useState<string | null>(null)

	const requestIdRef = useRef(0)
	const abortRef = useRef<AbortController | null>(null)

	const abort = useCallback(() => {
		abortRef.current?.abort()
		abortRef.current = null
	}, [])

	const run = useCallback(
		async (q: string, cursor?: string) => {
			abort()
			const controller = new AbortController()
			abortRef.current = controller
			const requestId = ++requestIdRef.current
			if (cursor) {
				setIsLoadingMore(true)
			} else {
				setIsSearching(true)
				setError(null)
			}
			try {
				const response = await sdk.pdf.search({
					id: mediaId,
					q,
					cursor,
					limit: 50,
					signal: controller.signal,
				})
				if (requestId !== requestIdRef.current) return
				setResults((previous) => (cursor ? [...previous, ...response.results] : response.results))
				setNextCursor(response.nextCursor ?? null)
				setHasSearched(true)
			} catch (err) {
				if (controller.signal.aborted || requestId !== requestIdRef.current) return
				console.error('[PdfSearchDialog] search failed', err)
				setError(translate('shared.epubReader.search.failed'))
			} finally {
				if (requestId === requestIdRef.current) {
					setIsSearching(false)
					setIsLoadingMore(false)
				}
			}
		},
		[abort, mediaId, sdk, translate],
	)

	const submit = useCallback(() => {
		const trimmed = query.trim()
		if (trimmed.length < 2) return
		void run(trimmed)
	}, [query, run])

	useEffect(() => {
		if (!query.trim()) {
			abort()
			setResults([])
			setNextCursor(null)
			setHasSearched(false)
			setError(null)
		}
	}, [query, abort])

	useEffect(() => {
		if (!open) abort()
	}, [open, abort])

	useEffect(() => () => abort(), [abort])

	const renderContent = () => {
		if (isSearching) {
			return (
				<div className="h-32 flex w-full items-center justify-center">
					<Spinner />
				</div>
			)
		}
		if (!hasSearched) return null
		if (error) return <Command.Empty>{error}</Command.Empty>
		if (!results.length) {
			return <Command.Empty>{translate('shared.epubReader.search.noResults')}</Command.Empty>
		}

		return (
			<>
				{results.map((result, index) => {
					const [start, end] = result.match
					const value = `${result.page}:${result.chars[0]}:${index}`
					return (
						<Command.Item
							key={value}
							value={value}
							onSelect={() => onSelect(result)}
							className="gap-1 flex flex-col items-start"
						>
							<span className="text-xs text-muted-foreground">
								{translate('shared.readerAnnotations.page', { page: labelOf(result.page) })}
							</span>
							<p className="w-full">
								{result.snippet.slice(0, start)}
								<span className="bg-yellow-400 text-gray-900">
									{result.snippet.slice(start, end)}
								</span>
								{result.snippet.slice(end)}
							</p>
						</Command.Item>
					)
				})}
				{nextCursor && (
					<Command.Item
						value="__load-more__"
						disabled={isLoadingMore}
						onSelect={() => void run(query.trim(), nextCursor)}
						className="justify-center text-center text-muted-foreground"
					>
						{isLoadingMore ? (
							<span className="gap-x-2 flex items-center">
								<Loader2 className="h-3 w-3 animate-spin" />
								{translate('shared.epubReader.search.loadingMore')}
							</span>
						) : (
							translate('shared.epubReader.search.loadMore')
						)}
					</Command.Item>
				)}
			</>
		)
	}

	return (
		<Command.Dialog open={open} onOpenChange={onOpenChange}>
			<div className="px-4 flex items-center border-b border-b-border">
				<Search className="mr-2 h-4 w-4 shrink-0 text-muted-foreground opacity-50" />
				<input
					autoFocus
					placeholder={translate('shared.epubReader.search.placeholder')}
					aria-label={translate('shared.readerAnnotations.shortcuts.search')}
					className={cn(
						'h-11 py-3 text-sm flex w-full rounded-md bg-transparent text-foreground outline-none placeholder:text-muted-foreground',
					)}
					value={query}
					onChange={(event) => setQuery(event.target.value)}
					onKeyDown={(event) => {
						if (event.key === 'Enter') {
							event.preventDefault()
							event.stopPropagation()
							submit()
						}
					}}
				/>
			</div>
			<Command.List>{renderContent()}</Command.List>
		</Command.Dialog>
	)
}
