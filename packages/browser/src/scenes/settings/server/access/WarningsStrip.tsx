import { Card, Text } from '@stump/components'
import { AccessWarningKind, AccessWarningSeverity } from '@stump/graphql'
import { AlertTriangle, ShieldAlert } from 'lucide-react'

import { useTranslate } from '@/hooks/useTranslate'

import { formatWhen } from './format'
import { type AccessWarning, useAccessWarnings } from './graphql'
import { accessKey } from './keys'

/** The warning's sentence in the locale files, and the part of the page that deals with it */
const KINDS: Record<AccessWarningKind, { key: string; unknownKey?: string; section: string }> = {
	[AccessWarningKind.AtDeviceLimit]: { key: 'atDeviceLimit', section: 'people' },
	[AccessWarningKind.OverDeviceLimit]: { key: 'overDeviceLimit', section: 'people' },
	[AccessWarningKind.ManyDevices]: { key: 'manyDevices', section: 'devices' },
	[AccessWarningKind.DeviceLimitHit]: { key: 'deviceLimitHit', section: 'people' },
	[AccessWarningKind.ManySessions]: { key: 'manySessions', section: 'people' },
	[AccessWarningKind.SigninFailures]: {
		key: 'signinFailures',
		unknownKey: 'signinFailuresUnknown',
		section: 'signins',
	},
	[AccessWarningKind.NameBackoff]: {
		key: 'nameBackoff',
		unknownKey: 'nameBackoffUnknown',
		section: 'signins',
	},
	[AccessWarningKind.GlobalBudget]: { key: 'globalBudget', section: 'signins' },
	[AccessWarningKind.NotEnabledTried]: { key: 'notEnabledTried', section: 'people' },
	[AccessWarningKind.NotFriendClass]: { key: 'notFriendClass', section: 'people' },
	[AccessWarningKind.LinkNearBudget]: { key: 'linkNearBudget', section: 'signups' },
	[AccessWarningKind.LinkNoLibraries]: { key: 'linkNoLibraries', section: 'signups' },
}

/** The sentence for one warning, with the person, count or note it is about filled in */
export function useWarningText() {
	const { t } = useTranslate()
	return (warning: Pick<AccessWarning, 'kind' | 'person' | 'count' | 'detail'>) => {
		const kind = KINDS[warning.kind]
		const key = !warning.person && kind.unknownKey ? kind.unknownKey : kind.key
		return t(accessKey(`warnings.kinds.${key}`), {
			name: warning.person?.username ?? '',
			count: warning.count,
			detail: warning.detail ?? '',
		})
	}
}

/**
 * What needs the owner's attention, at the top of the page: people at or over their device limit,
 * many sessions, bursts of failed sign-ins, links nearly out of budget. Nothing at all when
 * there is nothing to say. Each line says what it is and where on the page to deal with it
 */
export default function WarningsStrip() {
	const { t } = useTranslate()
	const warnings = useAccessWarnings()
	const text = useWarningText()

	if (warnings.length === 0) return null

	return (
		<section
			data-testid="access-warnings"
			aria-label={t(accessKey('warnings.title'))}
			className="gap-3 flex flex-col"
		>
			<Text className="font-medium">
				{t(accessKey('warnings.title'), { count: warnings.length })}
			</Text>
			<Card className="divide-y divide-border">
				{warnings.map((warning) => {
					const isAlert = warning.severity === AccessWarningSeverity.Alert
					const Icon = isAlert ? ShieldAlert : AlertTriangle
					return (
						<div
							key={warning.id}
							data-testid={`access-warning-${warning.id}`}
							data-severity={warning.severity}
							className="gap-3 px-4 py-3 flex items-start"
						>
							<Icon
								aria-hidden
								className={isAlert ? 'mt-0.5 h-4 w-4 shrink-0 text-destructive' : 'mt-0.5 h-4 w-4 shrink-0 text-amber-500'}
							/>
							<div className="min-w-0 flex-1">
								<Text size="sm">{text(warning)}</Text>
								<Text size="xs" variant="muted">
									{t(accessKey('warnings.since'), { at: formatWhen(warning.since) })}
									{' · '}
									<a
										className="underline"
										href={`#access-${KINDS[warning.kind].section}`}
									>
										{t(accessKey('warnings.review'))}
									</a>
								</Text>
							</div>
						</div>
					)
				})}
			</Card>
		</section>
	)
}
