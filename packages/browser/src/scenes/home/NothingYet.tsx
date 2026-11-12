import { Text } from '@stump/components'
import { BookMarked } from 'lucide-react'

import { useTranslate } from '@/hooks/useTranslate'

/**
 * What a reader with nothing in progress and nothing next sees at the top of the home page, once,
 * in place of an empty box for each of the sections that have nothing to show
 */
export default function NothingYet() {
	const { t } = useTranslate()

	return (
		<div
			role="status"
			className="space-x-3 px-4 py-3 flex items-center justify-start rounded-lg border border-dashed border-border"
		>
			<span className="p-2 rounded-lg border border-border bg-muted">
				<BookMarked className="h-6 w-6 text-muted-foreground" />
			</span>
			<div>
				<Text>{t('homeNothingYet.title')}</Text>
				<Text size="sm" variant="muted">
					{t('homeNothingYet.message')}
				</Text>
			</div>
		</div>
	)
}
