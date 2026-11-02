import { Dialog } from '@stump/components'

import { useTranslate } from '@/hooks/useTranslate'

import GoToField from '../shared/GoToField'

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
	return (
		<Dialog open={open} onOpenChange={onOpenChange}>
			<Dialog.Content size="sm">
				<Dialog.Header>
					<Dialog.Title>{translate('shared.pdfReader.goToPage')}</Dialog.Title>
					<Dialog.Close />
				</Dialog.Header>
				<GoToField
					label={translate('shared.pdfReader.goToPage')}
					placeholder={`1 – ${pageCount}`}
					resolve={resolve}
					invalidMessage={translate('shared.pdfReader.noSuchPage')}
					onGo={(page) => {
						onGo(page)
						onOpenChange(false)
					}}
					autoFocus={open}
					resetKey={open}
				/>
			</Dialog.Content>
		</Dialog>
	)
}
