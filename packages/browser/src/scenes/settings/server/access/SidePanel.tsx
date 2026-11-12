import { Button, Drawer, SheetPrimitive } from '@stump/components'
import { X } from 'lucide-react'
import type { PropsWithChildren, ReactNode } from 'react'

import { useTranslate } from '@/hooks/useTranslate'

import { accessKey } from './keys'
import { useIsWide } from './useIsWide'

type Props = PropsWithChildren<{
	open: boolean
	onClose: () => void
	title: string
	description?: ReactNode
	testId?: string
}>

/**
 * A detail inspector: a sheet from the right on a screen as wide as a tablet, a drawer from the
 * bottom on a phone. Driven by the caller's own state (it is not closed by the route changing, as
 * the ready-made `Sheet` is, because the page keeps what is open in the address)
 */
export default function SidePanel({ open, onClose, title, description, testId, children }: Props) {
	const wide = useIsWide()
	const { t } = useTranslate()
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
					<div className="gap-6 px-4 pb-8 flex flex-col">{children}</div>
				</SheetPrimitive.Content>
			</SheetPrimitive>
		)
	}

	return (
		<Drawer open={open} onOpenChange={change} shouldScaleBackground={false}>
			<Drawer.Content className="max-h-[90dvh]" data-testid={testId}>
				<Drawer.Header className="text-left">
					<div className="gap-2 flex items-center justify-between">
						<Drawer.Title>{title}</Drawer.Title>
						<Drawer.Close asChild>
							<Button variant="ghost" size="icon" aria-label={t(accessKey('close'))}>
								<X className="h-4 w-4" />
							</Button>
						</Drawer.Close>
					</div>
					{description && <Drawer.Description>{description}</Drawer.Description>}
				</Drawer.Header>
				<div className="gap-6 px-4 pb-8 flex flex-col overflow-y-auto">{children}</div>
			</Drawer.Content>
		</Drawer>
	)
}
