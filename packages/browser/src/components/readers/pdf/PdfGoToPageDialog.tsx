import { Dialog } from '@stump/components'

import { useTranslate } from '@/hooks/useTranslate'

import GoToField from '../shared/GoToField'
import type { PageNavHistory } from '../shared/navHistory'
import NavHistoryRow from '../shared/NavHistoryRow'

type Props = {
	open: boolean
	onOpenChange: (open: boolean) => void
	pageCount: number
	/** Resolves what was typed (a page label or number) to a 1-based page, or `null` */
	resolve: (input: string) => number | null
	onGo: (page: number) => void
	/** Back to the page left by the last jump (and forward again), above the field */
	history?: Pick<PageNavHistory, 'back' | 'forward' | 'goBack' | 'goForward'>
}

/** "Go to page": a page number, or the PDF's own page label (e.g. "xix") */
export default function PdfGoToPageDialog({
	open,
	onOpenChange,
	pageCount,
	resolve,
	onGo,
	history,
}: Props) {
	const { translate } = useTranslate()
	return (
		<Dialog open={open} onOpenChange={onOpenChange}>
			<Dialog.Content size="sm">
				<Dialog.Header>
					<Dialog.Title>{translate('shared.pdfReader.goToPage')}</Dialog.Title>
					<Dialog.Close />
				</Dialog.Header>
				{history && (
					<NavHistoryRow
						back={history.back}
						forward={history.forward}
						onBack={() => {
							history.goBack()
							onOpenChange(false)
						}}
						onForward={() => {
							history.goForward()
							onOpenChange(false)
						}}
						className="-mb-1"
					/>
				)}
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
