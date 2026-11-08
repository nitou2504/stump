import { useGraphQL } from '@stump/client'
import { Button, cn, Drawer, Input, Popover } from '@stump/components'
import { Check, ListChecks, ListPlus, Plus } from 'lucide-react'
import { FormEvent, useMemo, useState } from 'react'
import { useMediaMatch } from 'rooks'
import { toast } from 'sonner'

import { Link, useAppContext } from '@/context'
import { useTranslate } from '@/hooks/useTranslate'
import { usePaths } from '@/paths'

import { bookReadingListsQuery, byName, READING_LISTS_KEY, useReadingListActions } from './queries'

const errorMessage = (error: unknown) =>
	error instanceof Error && error.message ? error.message : undefined

/** One line of the menu: as tall as a finger on touch screens */
const ROW =
	'flex h-9 pointer-coarse:h-11 w-full items-center gap-3 rounded-md px-2 text-left text-sm text-foreground transition-colors hover:bg-accent/60 focus-visible:bg-accent/60 focus-visible:outline-none disabled:opacity-60'

type Props = {
	bookId: string
	bookName: string
}

/**
 * The book page's "Add to list" button: the user's reading lists with a check on the ones
 * that have the book (a tap adds or removes it) and a new list made on the spot with the book
 * in it. A popover under the button on wider screens, a sheet from the bottom on phones.
 */
export default function AddToReadingList({ bookId, bookName }: Props) {
	const { t } = useTranslate()
	const { user } = useAppContext()
	const isMobile = useMediaMatch('(max-width: 768px)')
	const [isOpen, setIsOpen] = useState(false)

	const { data, isLoading } = useGraphQL(
		bookReadingListsQuery,
		[READING_LISTS_KEY, 'book', bookId],
		{ id: bookId },
	)
	const inLists = useMemo(() => new Set(data?.mediaById?.readingListIds ?? []), [data])
	// only the user's own lists can change
	const lists = useMemo(
		() =>
			(data?.readingLists.nodes ?? [])
				.filter((list) => list.creatingUserId === user.id)
				.sort(byName),
		[data, user.id],
	)

	const label = t('readingLists.addToList.label')
	const isInAList = inLists.size > 0
	const Icon = isInAList ? ListChecks : ListPlus
	const trigger = (
		<Button
			variant="outline"
			size="icon"
			className="size-9 pointer-coarse:size-10"
			aria-label={label}
			title={label}
		>
			<Icon className={cn(isInAList && 'text-primary')} />
		</Button>
	)

	const picker = (
		<ReadingListPicker
			bookId={bookId}
			lists={lists}
			inLists={inLists}
			isLoading={isLoading}
			onLeave={() => setIsOpen(false)}
		/>
	)

	if (isMobile) {
		return (
			<Drawer open={isOpen} onOpenChange={setIsOpen} shouldScaleBackground={false}>
				<Drawer.Trigger asChild>{trigger}</Drawer.Trigger>
				<Drawer.Content className="max-h-[85dvh]">
					<Drawer.Header className="pb-2 text-left">
						<Drawer.Title>{label}</Drawer.Title>
						<Drawer.Description className="truncate">{bookName}</Drawer.Description>
					</Drawer.Header>
					<div className="px-2 overflow-y-auto pb-[calc(1rem+env(safe-area-inset-bottom))]">
						{picker}
					</div>
				</Drawer.Content>
			</Drawer>
		)
	}

	return (
		<Popover open={isOpen} onOpenChange={setIsOpen}>
			<Popover.Trigger asChild>{trigger}</Popover.Trigger>
			<Popover.Content portal align="end" className="w-72 p-1.5">
				<p className="px-2 pt-1 pb-1.5 text-xs font-medium text-muted-foreground">{label}</p>
				<div className="max-h-80 overflow-y-auto">{picker}</div>
			</Popover.Content>
		</Popover>
	)
}

type PickerProps = {
	bookId: string
	lists: { id: string; name: string; bookCount: number }[]
	inLists: Set<string>
	isLoading: boolean
	/** Closes the menu, when a link takes the user to another page */
	onLeave: () => void
}

function ReadingListPicker({ bookId, lists, inLists, isLoading, onLeave }: PickerProps) {
	const { t, translate } = useTranslate()
	const paths = usePaths()
	const { createList, addBook, removeBook, isSaving } = useReadingListActions()

	// the list being changed, shown as it will be until the change is saved
	const [pending, setPending] = useState<{ id: string; checked: boolean } | null>(null)
	const [isNaming, setIsNaming] = useState(false)
	const [name, setName] = useState('')

	const toggle = async (id: string, checked: boolean) => {
		setPending({ id, checked })
		try {
			if (checked) {
				await addBook(id, bookId)
			} else {
				await removeBook(id, bookId)
			}
		} catch (error) {
			toast.error(t('readingLists.errors.saveBook'), { description: errorMessage(error) })
		} finally {
			setPending(null)
		}
	}

	const create = async (event: FormEvent) => {
		event.preventDefault()
		const trimmed = name.trim()
		if (!trimmed) return
		try {
			await createList({ name: trimmed, mediaIds: [bookId] })
			setName('')
			setIsNaming(false)
		} catch (error) {
			toast.error(t('readingLists.errors.create'), { description: errorMessage(error) })
		}
	}

	return (
		<div className="gap-0.5 flex flex-col">
			{!isLoading && !lists.length && (
				<p className="px-2 py-2 text-sm text-muted-foreground">
					{t('readingLists.addToList.noLists')}
				</p>
			)}

			{lists.map((list) => {
				const checked = pending?.id === list.id ? pending.checked : inLists.has(list.id)
				return (
					<button
						key={list.id}
						type="button"
						role="checkbox"
						aria-checked={checked}
						disabled={pending?.id === list.id}
						onClick={() => toggle(list.id, !checked)}
						className={ROW}
					>
						<span
							className={cn(
								'size-4 flex shrink-0 items-center justify-center rounded-[4px] border transition-colors',
								checked
									? 'border-primary bg-primary text-primary-foreground'
									: 'border-muted-foreground/50',
							)}
						>
							{checked && <Check className="size-3" strokeWidth={3} />}
						</span>
						<span className="min-w-0 flex-1 truncate">{list.name}</span>
						<span className="text-xs text-muted-foreground tabular-nums">{list.bookCount}</span>
					</button>
				)
			})}

			<div className="my-1 h-px bg-border" />

			{isNaming ? (
				<form onSubmit={create} className="gap-2 px-1 py-1 flex items-center">
					<Input
						autoFocus
						fullWidth
						value={name}
						onChange={(event) => setName(event.target.value)}
						placeholder={t('readingLists.form.namePlaceholder')}
						aria-label={t('readingLists.form.name')}
						maxLength={200}
						containerClassName="flex-1"
					/>
					<Button type="submit" size="sm" disabled={!name.trim() || isSaving}>
						{translate('shared.common.create')}
					</Button>
				</form>
			) : (
				<button type="button" onClick={() => setIsNaming(true)} className={ROW}>
					<Plus className="size-4 shrink-0 text-muted-foreground" />
					<span>{t('readingLists.addToList.newList')}</span>
				</button>
			)}

			<Link
				to={paths.readingLists()}
				onClick={onLeave}
				className={cn(ROW, 'text-muted-foreground hover:text-foreground')}
			>
				<ListChecks className="size-4 shrink-0" />
				<span>{t('readingLists.addToList.allLists')}</span>
			</Link>
		</div>
	)
}
