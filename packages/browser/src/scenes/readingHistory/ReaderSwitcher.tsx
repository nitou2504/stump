import { Avatar, cn } from '@stump/components'

import { Link } from '@/context'
import { useTranslate } from '@/hooks/useTranslate'
import { usePaths } from '@/paths'

type Person = { id: string; username: string; avatarUrl?: string | null }

type Props = {
	/** The people who let everyone see their reading (the person asking isn't one of them) */
	friends: Person[]
	/** Whose page it is: a friend's id, or nothing for the person's own */
	current?: string
}

/**
 * Whose reading to look at: the person's own, and each friend who lets them see theirs. With
 * nobody sharing there is nothing to choose, so it isn't shown
 */
export default function ReaderSwitcher({ friends, current }: Props) {
	const { t } = useTranslate()
	const paths = usePaths()

	if (!friends.length) return null

	const items: { key: string; label: string; to: string; person?: Person; isActive: boolean }[] = [
		{ key: 'me', label: t('readingHistory.people.me'), to: paths.myReading(), isActive: !current },
		...friends.map((friend) => ({
			key: friend.id,
			label: friend.username,
			to: paths.myReading(friend.id),
			person: friend,
			isActive: current === friend.id,
		})),
	]

	return (
		<nav
			aria-label={t('readingHistory.people.label')}
			className="gap-2 flex flex-wrap items-center"
		>
			{items.map(({ key, label, to, person, isActive }) => (
				<Link
					key={key}
					to={to}
					aria-current={isActive ? 'page' : undefined}
					className={cn(
						'gap-2 px-3 py-1.5 text-sm flex items-center rounded-full border border-border transition-colors hover:bg-accent',
						isActive && 'font-medium border-transparent bg-secondary text-foreground',
					)}
				>
					{person && (
						<Avatar
							className="size-5"
							src={person.avatarUrl ?? undefined}
							fallback={person.username}
						/>
					)}
					{label}
				</Link>
			))}
		</nav>
	)
}
