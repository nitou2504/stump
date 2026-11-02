import { Button, Dialog, Input } from '@stump/components'
import { useEffect, useState } from 'react'

import { useTranslate } from '@/hooks/useTranslate'

type Props = {
	open: boolean
	onOpenChange: (open: boolean) => void
	pageCount: number
	/** Resolves what was typed (a page label or number) to a 1-based page, or `null` */
	resolve: (input: string) => number | null
	onGo: (page: number) => void
}

/** "Go to page": a page number, or the PDF's own page label (e.g. "xix") */
export default function PdfGoToPageDialog({ open, onOpenChange, pageCount, resolve, onGo }: Props) {
	const { translate } = useTranslate()
	const [value, setValue] = useState('')
	const [invalid, setInvalid] = useState(false)

	useEffect(() => {
		if (open) {
			setValue('')
			setInvalid(false)
		}
	}, [open])

	const submit = () => {
		const page = resolve(value)
		if (page == null) {
			setInvalid(true)
			return
		}
		onGo(page)
		onOpenChange(false)
	}

	return (
		<Dialog open={open} onOpenChange={onOpenChange}>
			<Dialog.Content size="sm">
				<Dialog.Header>
					<Dialog.Title>{translate('shared.pdfReader.goToPage')}</Dialog.Title>
					<Dialog.Close />
				</Dialog.Header>
				<form
					className="gap-2 flex items-start"
					onSubmit={(event) => {
						event.preventDefault()
						submit()
					}}
				>
					<div className="flex-1">
						<Input
							autoFocus
							inputMode="numeric"
							aria-label={translate('shared.pdfReader.goToPage')}
							placeholder={`1 – ${pageCount}`}
							value={value}
							onChange={(event) => {
								setValue(event.target.value)
								setInvalid(false)
							}}
							errorMessage={invalid ? translate('shared.pdfReader.noSuchPage') : undefined}
						/>
					</div>
					<Button type="submit">{translate('shared.pdfReader.go')}</Button>
				</form>
			</Dialog.Content>
		</Dialog>
	)
}
