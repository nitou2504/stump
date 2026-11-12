import { Button, Dialog, Drawer } from '@stump/components'
import { X } from 'lucide-react'
import type { PropsWithChildren } from 'react'

import { useTranslate } from '@/hooks/useTranslate'

import { accessKey } from './keys'
import { useIsWide } from './useIsWide'

type Props = PropsWithChildren<{
	open: boolean
	onClose: () => void
	title: string
	testId?: string
}>

/** A dialog on a screen as wide as a tablet, a drawer that fills the phone's screen on a phone */
export default function ModalPanel({ open, onClose, title, testId, children }: Props) {
	const wide = useIsWide()
	const { t } = useTranslate()

	if (wide) {
		return (
			<Dialog open={open} onOpenChange={(next) => !next && onClose()}>
				<Dialog.Content size="md" data-testid={testId} className="max-h-[90dvh] overflow-y-auto">
					<Dialog.Header>
						<Dialog.Title>{title}</Dialog.Title>
						<Dialog.Close onClick={onClose} />
					</Dialog.Header>
					{children}
				</Dialog.Content>
			</Dialog>
		)
	}

	return (
		<Drawer open={open} onOpenChange={(next) => !next && onClose()} shouldScaleBackground={false}>
			<Drawer.Content className="h-[96dvh] max-h-[96dvh]" data-testid={testId}>
				<Drawer.Header className="gap-2 flex flex-row items-center justify-between text-left">
					<Drawer.Title>{title}</Drawer.Title>
					<Drawer.Close asChild>
						<Button variant="ghost" size="icon" aria-label={t(accessKey('close'))}>
							<X className="h-4 w-4" />
						</Button>
					</Drawer.Close>
				</Drawer.Header>
				<div className="gap-6 px-4 pb-8 flex flex-col overflow-y-auto">{children}</div>
			</Drawer.Content>
		</Drawer>
	)
}
