import { useGraphQL } from '@stump/client'
import { Badge, Button, cn, Command, Popover } from '@stump/components'
import { IntakeSeriesKind } from '@stump/graphql'
import { Check, ChevronsUpDown, FolderPlus } from 'lucide-react'
import { useMemo, useState } from 'react'
import { useDebouncedValue } from 'rooks'

import { intakeFoldersQuery } from './queries'

type Candidate = {
	folder: string
	books: number
	confidence: number
	reason: string
	isNew: boolean
}

type Props = {
	libraryId: string
	/** null/undefined = author folders; set = folders under the manga / light novel root */
	seriesKind?: IntakeSeriesKind | null
	candidates: Candidate[]
	value: string
	onSelect: (folder: string) => void
	disabled?: boolean
}

const readable = (folder: string) => folder.replace(/_/g, ' ')

/**
 * Picks the author (or series) folder: the server's suggestions first, then the library's
 * existing folders as you type, and a "new folder" option for anything else.
 */
export default function FolderPicker({
	libraryId,
	seriesKind,
	candidates,
	value,
	onSelect,
	disabled,
}: Props) {
	const [open, setOpen] = useState(false)
	const [search, setSearch] = useState('')
	const [debounced] = useDebouncedValue(search, 250)

	const { data, isFetching } = useGraphQL(
		intakeFoldersQuery,
		['intakeFolders', libraryId, seriesKind ?? null, debounced],
		{ libraryId, limit: 20, search: debounced || undefined, seriesKind: seriesKind ?? undefined },
		{ enabled: open },
	)

	const existing = useMemo(() => {
		const suggested = new Set(candidates.map((c) => c.folder))
		return (data?.intakeFolders ?? []).filter((f) => !suggested.has(f.name))
	}, [data, candidates])

	const typed = search.trim()
	const typedMatches =
		!typed ||
		candidates.some((c) => c.folder.toLowerCase() === typed.toLowerCase()) ||
		existing.some((f) => f.name.toLowerCase() === typed.toLowerCase())

	const choose = (folder: string) => {
		setOpen(false)
		setSearch('')
		if (folder !== value) onSelect(folder)
	}

	return (
		<Popover open={open} onOpenChange={setOpen}>
			<Popover.Trigger asChild>
				<Button
					variant="outline"
					size="sm"
					disabled={disabled}
					className="gap-2 h-8 max-w-full justify-between"
					aria-label={seriesKind ? 'Change the series folder' : 'Change the author folder'}
				>
					<span className="truncate">{value ? readable(value) : 'Choose a folder'}</span>
					<ChevronsUpDown className="h-3.5 w-3.5 shrink-0 opacity-60" />
				</Button>
			</Popover.Trigger>
			<Popover.Content
				align="start"
				className="p-0 w-[min(24rem,calc(100vw-2rem))]"
				collisionPadding={12}
			>
				<Command shouldFilter={false}>
					<Command.Input
						placeholder={seriesKind ? 'Search series folders…' : 'Search author folders…'}
						value={search}
						onValueChange={setSearch}
					/>
					<Command.List className="max-h-[min(20rem,50vh)]">
						{candidates.length > 0 && (
							<Command.Group heading="Suggested">
								{candidates.map((c) => (
									<Command.Item
										key={`s:${c.folder}`}
										value={`s:${c.folder}`}
										onSelect={() => choose(c.folder)}
										className="gap-2 items-start"
									>
										<Check
											className={cn(
												'mt-0.5 h-4 w-4 shrink-0',
												c.folder === value ? 'opacity-100' : 'opacity-0',
											)}
										/>
										<div className="min-w-0 flex flex-1 flex-col">
											<span className="gap-2 flex items-center">
												<span className="truncate">{readable(c.folder)}</span>
												{c.isNew ? (
													<Badge variant="warning" size="xs">
														new folder
													</Badge>
												) : (
													<span className="text-xs text-muted-foreground">{c.books} books</span>
												)}
											</span>
											<span className="text-xs truncate text-muted-foreground">{c.reason}</span>
										</div>
									</Command.Item>
								))}
							</Command.Group>
						)}

						{existing.length > 0 && (
							<Command.Group heading={typed ? 'Library folders' : 'Biggest folders'}>
								{existing.map((f) => (
									<Command.Item
										key={`e:${f.name}`}
										value={`e:${f.name}`}
										onSelect={() => choose(f.name)}
										className="gap-2"
									>
										<Check
											className={cn(
												'h-4 w-4 shrink-0',
												f.name === value ? 'opacity-100' : 'opacity-0',
											)}
										/>
										<span className="flex-1 truncate">{readable(f.name)}</span>
										<span className="text-xs text-muted-foreground">{f.books}</span>
									</Command.Item>
								))}
							</Command.Group>
						)}

						{!typedMatches && (
							<Command.Group heading="New">
								<Command.Item value={`n:${typed}`} onSelect={() => choose(typed)} className="gap-2">
									<FolderPlus className="h-4 w-4 shrink-0" />
									<span className="truncate">
										New folder “{typed}”
										{!seriesKind && (
											<span className="text-xs text-muted-foreground">
												{' '}
												(named the library’s way)
											</span>
										)}
									</span>
								</Command.Item>
							</Command.Group>
						)}

						{!candidates.length && !existing.length && typedMatches && (
							<div className="py-6 text-sm text-center text-muted-foreground">
								{isFetching ? 'Searching…' : 'Type a name to search'}
							</div>
						)}
					</Command.List>
				</Command>
			</Popover.Content>
		</Popover>
	)
}
