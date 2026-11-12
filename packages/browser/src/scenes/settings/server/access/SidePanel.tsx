import { Drawer, SheetPrimitive } from '@stump/components'
import type { PropsWithChildren } from 'react'

import { useIsWide } from './useIsWide'

type Props = PropsWithChildren<{
	open: boolean
	onClose: () => void
	title: string
	description?: string
	testId?: string
}>

/**
 * A detail inspector: a sheet from the right on a screen as wide as a tablet, a drawer from the
 * bottom on a phone. Driven by the caller's own state (it is not closed by the route changing, as
 * the ready-made `Sheet` is, because the page keeps what is open in the address)
 */
export default function SidePanel({ open, onClose, title, description, testId, children }: Props) {
	const wide = useIsWide()
	const change = (next: boolean) => {
		if (!next) onClose()
	}

	if (wide) {
		return (
			<SheetPrimitive open={open} onOpenChange={change}>
				<SheetPrimitive.Content
					closeIcon
					position="right"
					size="lg"
					data-testid={testId}
					className="gap-6 overflow-y-auto"
				>
					<SheetPrimitive.Header>
						<SheetPrimitive.Title>{title}</SheetPrimitive.Title>
						{description && <SheetPrimitive.Description>{description}</SheetPrimitive.Description>}
					</SheetPrimitive.Header>
					{children}
				</SheetPrimitive.Content>
			</SheetPrimitive>
		)
	}

	return (
		<Drawer open={open} onOpenChange={change} shouldScaleBackground={false}>
			<Drawer.Content className="max-h-[90dvh]" data-testid={testId}>
				<Drawer.Header className="text-left">
					<Drawer.Title>{title}</Drawer.Title>
					{description && <Drawer.Description>{description}</Drawer.Description>}
				</Drawer.Header>
				<div className="gap-6 px-4 pb-8 flex flex-col overflow-y-auto">{children}</div>
			</Drawer.Content>
		</Drawer>
	)
}
