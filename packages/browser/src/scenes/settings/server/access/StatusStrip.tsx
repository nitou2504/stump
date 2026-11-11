import { Alert, AlertDescription, AlertTitle, Badge, Card, Text } from '@stump/components'
import { AlertTriangle } from 'lucide-react'
import type { ReactNode } from 'react'

import { useTranslate } from '@/hooks/useTranslate'

import { formatWhen } from './format'
import { useAccessOverview } from './graphql'
import { accessKey } from './keys'

function Fact({ label, children }: { label: string; children: ReactNode }) {
	return (
		<div className="min-w-0">
			<dt className="text-xs text-muted-foreground">{label}</dt>
			<dd className="text-sm font-medium break-words">{children}</dd>
		</div>
	)
}

/**
 * What is set up for people to come in from the internet and what is missing, at a glance. When
 * something is missing, invites can't be made, so it is said first and in full
 */
export default function StatusStrip() {
	const { t } = useTranslate()
	const overview = useAccessOverview()

	// the names are the settings' own, as the deploy notes call them
	const missing = [
		!overview.hasKey && 'STUMP_EDGE_KEY',
		!overview.hasPublicUrl && 'STUMP_EDGE_PUBLIC_URL',
		!overview.hasStateDir && 'STUMP_EDGE_STATE_DIR',
	].filter(Boolean) as string[]

	return (
		<div data-testid="access-status" className="gap-y-4 flex flex-col">
			{!overview.configured && (
				<Alert variant="warning">
					<AlertTriangle />
					<AlertTitle>{t(accessKey('status.notConfigured.title'))}</AlertTitle>
					<AlertDescription>
						{t(accessKey('status.notConfigured.description'), { missing: missing.join(', ') })}
					</AlertDescription>
				</Alert>
			)}

			<Card className="p-4">
				<div className="gap-x-8 gap-y-4 flex flex-wrap items-center">
					<Badge size="lg" variant={overview.configured ? 'success' : 'warning'}>
						{t(accessKey(overview.configured ? 'status.ready' : 'status.notReady'))}
					</Badge>
					<dl className="gap-x-8 gap-y-3 flex flex-1 flex-wrap">
						<Fact label={t(accessKey('status.publicUrl'))}>
							{overview.publicUrl ?? t(accessKey('status.missing'))}
						</Fact>
						<Fact label={t(accessKey('status.requirePass'))}>
							{t(accessKey(overview.requirePass ? 'status.yes' : 'status.notYet'))}
						</Fact>
						<Fact label={t(accessKey('status.stateWritten'))}>
							{overview.stateWrittenAt
								? formatWhen(overview.stateWrittenAt)
								: t(accessKey('status.never'))}
						</Fact>
						<Fact label={t(accessKey('status.activeDevices'))}>{overview.activeDevices}</Fact>
						<Fact label={t(accessKey('status.openInvites'))}>{overview.openInvites}</Fact>
					</dl>
				</div>
				{overview.configured && !overview.requirePass && (
					<Text size="sm" variant="muted" className="mt-3">
						{t(accessKey('status.requirePassHint'))}
					</Text>
				)}
			</Card>
		</div>
	)
}
