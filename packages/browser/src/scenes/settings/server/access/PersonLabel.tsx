import { Badge, Text } from '@stump/components'

import { useTranslate } from '@/hooks/useTranslate'

import { accessKey } from './keys'

type Props = {
	person?: { username: string; removed?: boolean } | null
	/** What to say when there is no person (a pass made in the terminal belongs to nobody) */
	fallback?: string
}

/** A person's name; an account that was removed says so, since what it had is still listed */
export default function PersonLabel({ person, fallback }: Props) {
	const { t } = useTranslate()
	if (!person) {
		return (
			<Text size="sm" variant="muted">
				{fallback ?? '–'}
			</Text>
		)
	}

	return (
		<span className="gap-2 inline-flex items-center">
			<Text size="sm" className="font-medium" variant={person.removed ? 'muted' : 'default'}>
				{person.username}
			</Text>
			{person.removed && (
				<Badge size="xs" variant="secondary">
					{t(accessKey('person.removed'))}
				</Badge>
			)}
		</span>
	)
}
