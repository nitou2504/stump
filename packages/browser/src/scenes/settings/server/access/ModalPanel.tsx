import { Dialog, Drawer } from '@stump/components'
import type { PropsWithChildren } from 'react'

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

	if (wide) {
		return (
			<Dialog open={open} onOpenChange={(next) => !next && onClose()}>
				<Dialog.Content size="md" data-testid={testId}>
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
				<Drawer.Header className="text-left">
					<Drawer.Title>{title}</Drawer.Title>
				</Drawer.Header>
				<div className="gap-6 px-4 pb-8 flex flex-col overflow-y-auto">{children}</div>
			</Drawer.Content>
		</Drawer>
	)
}
