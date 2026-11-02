import { cn, IconButton } from '@stump/components'
import { ComponentProps, forwardRef } from 'react'

const ControlButton = forwardRef<HTMLButtonElement, ComponentProps<typeof IconButton>>(
	({ className, ...props }, ref) => {
		return (
			<IconButton
				variant="ghost"
				size="sm"
				className={cn('hover:bg-accent', className)}
				ref={ref}
				{...props}
			/>
		)
	},
)
ControlButton.displayName = 'ControlButton'

export default ControlButton
