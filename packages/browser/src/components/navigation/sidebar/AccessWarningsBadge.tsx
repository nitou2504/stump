import { cn } from '@stump/components'

import { useAppContext } from '@/context'
import { useTranslate } from '@/hooks/useTranslate'
import { useAccessWarningCount } from '@/scenes/settings/server/access/useWarningCount'

type Props = {
	/**
	 * `corner`: a small round badge on the top right of a button (the sidebar's settings button).
	 * `inline`: a pill after a label (the settings navigation).
	 * `dot`: a small dot with no number, on a button that has no room for one (the phone header's
	 * sidebar button)
	 */
	variant?: 'corner' | 'inline' | 'dot'
	className?: string
}

/**
 * How many things *Public access* has for the owner to look at: the items of *Needs your
 * attention*, which include the books that wait for approval. Only the server owner sees it, and
 * nothing is shown when there is nothing to look at (or after *Clear all*). It is amber, to tell
 * it from the approvals badge of those who approve but are not the owner
 */
export default function AccessWarningsBadge({ variant = 'corner', className }: Props) {
	const { isServerOwner } = useAppContext()
	const { t } = useTranslate()
	const count = useAccessWarningCount(isServerOwner)

	if (!count) return null

	const text = t('settingsScene.server/access.attention.badge', { count })

	if (variant === 'dot') {
		return (
			<>
				<span
					aria-hidden
					data-testid="access-warnings-dot"
					className={cn(
						'-right-0.5 -top-0.5 h-2.5 w-2.5 bg-amber-500 absolute rounded-full ring-2 ring-background',
						className,
					)}
				/>
				<span className="sr-only">{`, ${text}`}</span>
			</>
		)
	}

	return (
		<>
			<span
				aria-hidden
				title={text}
				data-testid="access-warnings-badge"
				className={cn(
					'bg-amber-500 font-semibold text-black',
					variant === 'corner'
						? '-right-1 -top-1 h-4 min-w-4 px-1 leading-4 absolute rounded-full text-center text-[10px]'
						: 'ml-2 h-4 min-w-4 px-1 leading-4 inline-block rounded-full text-center text-[10px]',
					className,
				)}
			>
				{count > 99 ? '99+' : count}
			</span>
			<span className="sr-only">{text}</span>
		</>
	)
}
