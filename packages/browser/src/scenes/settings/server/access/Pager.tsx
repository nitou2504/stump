import { Button, Text } from '@stump/components'

import { useTranslate } from '@/hooks/useTranslate'

import { accessKey } from './keys'

type Props = {
	/** The page shown, from 1 */
	page: number
	totalPages: number
	onChange: (page: number) => void
}

/** Previous and next for a list of pages; nothing at all when there is a single page */
export default function Pager({ page, totalPages, onChange }: Props) {
	const { t } = useTranslate()
	if (totalPages <= 1) return null

	return (
		<nav aria-label={t(accessKey('pager.label'))} className="gap-3 flex items-center justify-end">
			<Text size="sm" variant="muted">
				{t(accessKey('pager.page'), { page, total: totalPages })}
			</Text>
			<Button size="sm" variant="outline" disabled={page <= 1} onClick={() => onChange(page - 1)}>
				{t(accessKey('pager.previous'))}
			</Button>
			<Button
				size="sm"
				variant="outline"
				disabled={page >= totalPages}
				onClick={() => onChange(page + 1)}
			>
				{t(accessKey('pager.next'))}
			</Button>
		</nav>
	)
}
