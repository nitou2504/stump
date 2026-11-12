import { cn } from '@stump/components'

import { useAppContext } from '@/context'
import { useTranslate } from '@/hooks/useTranslate'
import { useAccessWarningCount } from '@/scenes/settings/server/access/useWarningCount'

type Props = {
	/**
	 * `corner`: a small round badge on the corner of a button (the sidebar's settings button).
	 * `inline`: a pill after a label (the settings navigation)
	 */
	variant?: 'corner' | 'inline'
	className?: string
}

/**
 * How many things *Public access* has for the owner to look at (people at their device limit,
 * bursts of failed sign-ins, links nearly out of budget...). Only the server owner sees it, and
 * nothing is shown when there is nothing to look at. It is amber, to tell it from the approvals badge
 */
export default function AccessWarningsBadge({ variant = 'corner', className }: Props) {
	const { isServerOwner } = useAppContext()
	const { t } = useTranslate()
	const count = useAccessWarningCount(isServerOwner)

	if (!count) return null

	return (
		<>
			<span
				aria-hidden
				data-testid="access-warnings-badge"
				className={cn(
					'bg-amber-500 font-semibold text-black',
					variant === 'corner'
						? '-bottom-1 -right-1 h-4 min-w-4 px-1 text-[10px] leading-4 absolute rounded-full text-center'
						: 'ml-2 h-4 min-w-4 px-1 text-[10px] leading-4 inline-block rounded-full text-center',
					className,
				)}
			>
				{count > 99 ? '99+' : count}
			</span>
			<span className="sr-only">{t('settingsScene.server/access.warnings.badge', { count })}</span>
		</>
	)
}
