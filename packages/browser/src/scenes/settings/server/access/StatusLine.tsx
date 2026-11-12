import { Alert, AlertDescription, AlertTitle, Button, Text } from '@stump/components'
import { AlertTriangle, CheckCircle2 } from 'lucide-react'
import { useState } from 'react'

import { useTranslate } from '@/hooks/useTranslate'

import DetailsDialog from './DetailsDialog'
import { useAccessOverview, useAccessPeople } from './graphql'
import { accessKey } from './keys'

/** The host of the public address, which is what the owner knows it by */
export function hostOf(url?: string | null): string {
	if (!url) return ''
	try {
		return new URL(url).host
	} catch {
		return url
	}
}

/**
 * Is it working: one sentence at the top of the page. Everything technical (the numbers, the
 * address in full, when the edge was last told) is behind *Details*. When something is missing
 * and nobody could be invited, that replaces the sentence, in full
 */
export default function StatusLine() {
	const { t } = useTranslate()
	const overview = useAccessOverview()
	const people = useAccessPeople()
	const [details, setDetails] = useState(false)
	const host = hostOf(overview.publicUrl)
	const inside = people.filter((row) => row.publicSignin).length

	// the names the owner knows them by, not the settings' own
	const missing = [
		!overview.hasKey && t(accessKey('status.missingNames.key')),
		!overview.hasPublicUrl && t(accessKey('status.missingNames.publicUrl')),
		!overview.hasStateDir && t(accessKey('status.missingNames.stateDir')),
	].filter(Boolean) as string[]

	return (
		<div data-testid="access-status" className="gap-3 flex flex-col">
			{!overview.configured ? (
				<Alert variant="warning">
					<AlertTriangle />
					<AlertTitle>{t(accessKey('status.notConfigured.title'))}</AlertTitle>
					<AlertDescription>
						{t(accessKey('status.notConfigured.description'), { missing: missing.join(', ') })}
					</AlertDescription>
				</Alert>
			) : (
				<div className="gap-x-3 gap-y-1 flex items-start justify-between">
					<div className="gap-2 min-w-0 flex items-start">
						{overview.signinStats.globalBudgetTripped ? (
							<span
								aria-hidden
								className="mt-1.5 h-2.5 w-2.5 shrink-0 rounded-full bg-destructive"
							/>
						) : (
							<CheckCircle2 aria-hidden className="mt-0.5 h-4 w-4 text-green-500 shrink-0" />
						)}
						<Text size="sm" data-testid="access-status-line">
							{overview.signinStats.globalBudgetTripped
								? t(accessKey('status.refusing'))
								: overview.publicSignin
									? `${t(accessKey('status.open'), { host })} ${t(accessKey('status.people'), { count: inside })}`
									: t(accessKey('status.linksOnly'), { host })}
						</Text>
					</div>
					<Button size="sm" variant="ghost" onClick={() => setDetails(true)}>
						{t(accessKey('status.details'))}
					</Button>
				</div>
			)}
			<DetailsDialog isOpen={details} onClose={() => setDetails(false)} />
		</div>
	)
}
