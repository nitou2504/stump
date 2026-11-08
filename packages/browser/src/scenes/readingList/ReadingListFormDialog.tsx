import { Button, Dialog, Input, TextArea } from '@stump/components'
import { FormEvent, useEffect, useState } from 'react'
import { toast } from 'sonner'

import { useReadingListActions } from '@/components/readingList/queries'
import { useTranslate } from '@/hooks/useTranslate'

type Props = {
	/** The list to edit; a new list is made without it */
	list?: { id: string; name: string; description?: string | null }
	isOpen: boolean
	onClose: () => void
}

/** Names a new reading list, or renames one and changes its description */
export default function ReadingListFormDialog({ list, isOpen, onClose }: Props) {
	const { t, translate } = useTranslate()
	const { createList, updateList, isSaving } = useReadingListActions()

	const [name, setName] = useState('')
	const [description, setDescription] = useState('')
	// start from the list's values each time the dialog opens
	useEffect(() => {
		if (isOpen) {
			setName(list?.name ?? '')
			setDescription(list?.description ?? '')
		}
	}, [isOpen, list?.name, list?.description])

	const handleSubmit = async (event: FormEvent) => {
		event.preventDefault()
		const input = { name: name.trim(), description: description.trim() }
		if (!input.name) return
		try {
			if (list) {
				await updateList(list.id, input)
			} else {
				await createList(input)
				toast.success(t('readingLists.created', { name: input.name }))
			}
			onClose()
		} catch (error) {
			toast.error(t(list ? 'readingLists.errors.update' : 'readingLists.errors.create'), {
				description: error instanceof Error ? error.message : undefined,
			})
		}
	}

	return (
		<Dialog open={isOpen} onOpenChange={(nowOpen) => !nowOpen && onClose()}>
			<Dialog.Content size="sm">
				<Dialog.Header>
					<Dialog.Title>
						{t(list ? 'readingLists.form.editTitle' : 'readingLists.form.createTitle')}
					</Dialog.Title>
					<Dialog.Close onClick={onClose} />
				</Dialog.Header>

				<form id="reading-list-form" onSubmit={handleSubmit} className="gap-4 flex flex-col">
					<Input
						id="reading-list-name"
						label={t('readingLists.form.name')}
						placeholder={t('readingLists.form.namePlaceholder')}
						value={name}
						onChange={(event) => setName(event.target.value)}
						maxLength={200}
						required
						autoFocus
						fullWidth
					/>
					<TextArea
						id="reading-list-description"
						label={t('readingLists.form.description')}
						placeholder={t('readingLists.form.descriptionPlaceholder')}
						value={description}
						onChange={(event) => setDescription(event.target.value)}
						rows={3}
					/>
				</form>

				<Dialog.Footer>
					<Button variant="outline" onClick={onClose}>
						{translate('shared.common.cancel')}
					</Button>
					<Button type="submit" form="reading-list-form" disabled={!name.trim() || isSaving}>
						{list ? translate('shared.common.saveChanges') : translate('shared.common.create')}
					</Button>
				</Dialog.Footer>
			</Dialog.Content>
		</Dialog>
	)
}
