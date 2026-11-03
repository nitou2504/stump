import { useGraphQL, useGraphQLMutation } from '@stump/client'
import { Button, Dialog, Input, Label, NativeSelect, TextArea } from '@stump/components'
import { MetadataFacetField } from '@stump/graphql'
import { useQueryClient } from '@tanstack/react-query'
import { Sparkles } from 'lucide-react'
import { useEffect, useId, useMemo, useRef, useState } from 'react'
import { toast } from 'sonner'

import EditHistory from './EditHistory'
import { type DetailsForm, detailsForm, editInput } from './form'
import GenreSuggestPanel from './GenreSuggestPanel'
import { notifySaved } from './outcome'
import { editBookDetailsQuery, editMediaMetadataMutation } from './queries'
import ValueChipsField, { useVocabulary } from './ValueChipsField'

type Props = {
	mediaId: string
	isOpen: boolean
	onClose: () => void
	/** Open on the genres, e.g. from the pencil next to a book's genres */
	focus?: 'genres'
}

const AGE_OPTIONS = [
	{ label: 'Not set', value: '' },
	{ label: 'All ages', value: '0' },
	{ label: '13+', value: '13' },
	{ label: '16+', value: '16' },
	{ label: '18+', value: '18' },
]

/** The formats whose files carry these details */
const WRITABLE = ['epub', 'pdf', 'cbz']

/** A text field with suggestions from the values the library already uses */
function SuggestedInput({
	field,
	libraryId,
	listId,
	...props
}: React.ComponentProps<typeof Input> & {
	field: MetadataFacetField
	libraryId?: string
	listId: string
}) {
	const [focused, setFocused] = useState(false)
	const { entries } = useVocabulary(field, libraryId, focused)
	const options = useMemo(
		() =>
			[...entries]
				.sort((a, b) => b.count - a.count)
				.slice(0, 400)
				.map((e) => e.value),
		[entries],
	)
	return (
		<>
			<Input
				{...props}
				fullWidth
				list={listId}
				onFocus={() => setFocused(true)}
				autoComplete="off"
			/>
			<datalist id={listId}>
				{options.map((value) => (
					<option key={value} value={value} />
				))}
			</datalist>
		</>
	)
}

/**
 * Edit a book's main details: title, authors, series, genres (picked from the library's
 * own genres, with suggestions), summary and publication details. Saving writes them into
 * the book file, so they survive rescans and the curator sees them; every save is in the
 * book's edit history, where it can be reverted.
 */
export default function EditBookDialog({ mediaId, isOpen, onClose, focus }: Props) {
	const id = useId()
	const client = useQueryClient()
	const { data } = useGraphQL(
		editBookDetailsQuery,
		['editBookDetails', mediaId],
		{ id: mediaId },
		{ enabled: isOpen },
	)
	const book = data?.mediaById
	const libraryId = book?.library.id
	const extension = (book?.extension ?? '').toLowerCase()

	const initial = useMemo<DetailsForm | null>(
		() => (book ? detailsForm(book.metadata) : null),
		[book],
	)

	const [form, setForm] = useState<DetailsForm | null>(null)
	const [suggesting, setSuggesting] = useState(false)
	useEffect(() => {
		if (isOpen && initial) {
			setForm(initial)
			setSuggesting(false)
		}
	}, [isOpen, initial])

	// opened from the genres: bring them into view once the form shows
	const genresRef = useRef<HTMLDivElement>(null)
	const scrolledRef = useRef(false)
	useEffect(() => {
		if (!isOpen) {
			scrolledRef.current = false
		} else if (focus === 'genres' && form && !scrolledRef.current) {
			scrolledRef.current = true
			genresRef.current?.scrollIntoView({ block: 'center' })
		}
	}, [focus, form, isOpen])

	const set = <K extends keyof DetailsForm>(key: K, value: DetailsForm[K]) =>
		setForm((f) => (f ? { ...f, [key]: value } : f))

	const input = useMemo(() => (initial && form ? editInput(initial, form) : {}), [initial, form])
	const isDirty = Object.keys(input).length > 0

	const { mutate, isPending } = useGraphQLMutation(editMediaMetadataMutation, {
		onError: (error) =>
			toast.error('The details could not be saved', {
				description: error instanceof Error ? error.message : undefined,
			}),
		onSuccess: ({ editMediaMetadata: { file } }) => {
			client.invalidateQueries()
			notifySaved(file)
			onClose()
		},
	})

	const onSave = () => {
		if (!form || !isDirty) return
		mutate({ id: mediaId, input })
	}

	const fileNote = WRITABLE.includes(extension)
		? `Saving writes the changes into the ${extension.toUpperCase()} file. Earlier versions stay in the edit history.`
		: 'This kind of file can’t hold these details: they are kept in Stump.'

	return (
		<Dialog open={isOpen} onOpenChange={(open) => !open && onClose()}>
			<Dialog.Content
				size="lg"
				className="max-h-[min(92dvh,56rem)] grid-rows-[auto_minmax(0,1fr)_auto] overflow-hidden"
				onEscapeKeyDown={(event) => {
					// Escape in a picker (a popover inside the dialog) closes the picker only
					if (
						event.target instanceof Element &&
						event.target.closest('[data-radix-popper-content-wrapper]')
					) {
						event.preventDefault()
					}
				}}
			>
				<Dialog.Header className="pr-8">
					<Dialog.Title>Edit details</Dialog.Title>
					<Dialog.Description className="line-clamp-1">
						{book?.resolvedName ?? 'Loading…'}
					</Dialog.Description>
					<Dialog.Close />
				</Dialog.Header>

				<div className="-mx-4 gap-4 px-4 pb-1 flex flex-col overflow-y-auto overscroll-contain">
					{!form && <p className="text-sm text-muted-foreground">Loading…</p>}
					{form && (
						<>
							<Input
								id={`${id}-title`}
								fullWidth
								label="Title"
								value={form.title}
								onChange={(e) => set('title', e.target.value)}
							/>

							<div className="gap-1.5 flex flex-col">
								<Label>Authors</Label>
								<ValueChipsField
									field={MetadataFacetField.Writers}
									libraryId={libraryId}
									values={form.writers}
									onChange={(writers) => set('writers', writers)}
									addLabel="Add an author"
								/>
							</div>

							<div className="gap-3 grid grid-cols-[minmax(0,1fr)_6rem]">
								<div className="gap-2 flex flex-col">
									<Label htmlFor={`${id}-series`}>Series</Label>
									<SuggestedInput
										id={`${id}-series`}
										field={MetadataFacetField.Series}
										libraryId={libraryId}
										listId={`${id}-series-list`}
										value={form.series}
										onChange={(e) => set('series', e.target.value)}
										placeholder="None"
									/>
								</div>
								<Input
									id={`${id}-number`}
									fullWidth
									label="Number"
									inputMode="decimal"
									value={form.number}
									onChange={(e) => set('number', e.target.value)}
									disabled={!form.series.trim()}
								/>
							</div>

							<div ref={genresRef} className="gap-2 scroll-mt-4 flex flex-col">
								<div className="gap-2 flex items-center justify-between">
									<Label>Genres</Label>
									{!suggesting && (
										<Button
											type="button"
											variant="outline"
											size="xs"
											onClick={() => setSuggesting(true)}
											autoFocus={focus === 'genres'}
										>
											<Sparkles /> Suggest genres
										</Button>
									)}
								</div>
								<ValueChipsField
									field={MetadataFacetField.Genres}
									libraryId={libraryId}
									values={form.genres}
									onChange={(genres) => set('genres', genres)}
									addLabel="Add a genre"
								/>
								{suggesting && (
									<GenreSuggestPanel
										mediaId={mediaId}
										current={form.genres}
										onApply={(genres) => {
											set('genres', genres)
											setSuggesting(false)
										}}
										onClose={() => setSuggesting(false)}
									/>
								)}
							</div>

							<TextArea
								id={`${id}-summary`}
								label="Summary"
								rows={5}
								value={form.summary}
								onChange={(e) => set('summary', e.target.value)}
							/>

							<div className="gap-3 sm:grid-cols-[minmax(0,1fr)_7rem] grid grid-cols-1">
								<div className="gap-2 flex flex-col">
									<Label htmlFor={`${id}-publisher`}>Publisher</Label>
									<SuggestedInput
										id={`${id}-publisher`}
										field={MetadataFacetField.Publisher}
										libraryId={libraryId}
										listId={`${id}-publisher-list`}
										value={form.publisher}
										onChange={(e) => set('publisher', e.target.value)}
									/>
								</div>
								<Input
									id={`${id}-year`}
									fullWidth
									label="Year"
									inputMode="numeric"
									maxLength={4}
									value={form.year}
									onChange={(e) => set('year', e.target.value.replace(/[^\d]/g, ''))}
								/>
							</div>

							<div className="gap-3 grid grid-cols-2">
								<div className="gap-2 flex flex-col">
									<Label htmlFor={`${id}-language`}>Language</Label>
									<SuggestedInput
										id={`${id}-language`}
										field={MetadataFacetField.Language}
										libraryId={libraryId}
										listId={`${id}-language-list`}
										value={form.language}
										onChange={(e) => set('language', e.target.value)}
										placeholder="en, es, …"
									/>
								</div>
								<div className="gap-2 flex flex-col">
									<Label htmlFor={`${id}-age`}>Age</Label>
									<NativeSelect
										id={`${id}-age`}
										options={
											AGE_OPTIONS.some((o) => o.value === form.ageRating)
												? AGE_OPTIONS
												: [...AGE_OPTIONS, { label: `${form.ageRating}+`, value: form.ageRating }]
										}
										value={form.ageRating}
										onChange={(e) => set('ageRating', e.target.value)}
									/>
								</div>
							</div>

							<EditHistory mediaId={mediaId} disabled={isDirty || isPending} />
						</>
					)}
				</div>

				<Dialog.Footer className="gap-3 sm:items-center sm:gap-2 sm:flex grid grid-cols-2">
					<p className="text-xs sm:mr-auto col-span-2 text-muted-foreground">{fileNote}</p>
					<Button type="button" variant="outline" onClick={onClose}>
						Cancel
					</Button>
					<Button type="button" onClick={onSave} disabled={!isDirty} isLoading={isPending}>
						Save
					</Button>
				</Dialog.Footer>
			</Dialog.Content>
		</Dialog>
	)
}
