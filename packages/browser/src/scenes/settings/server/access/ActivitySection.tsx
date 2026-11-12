import { cn, Text } from '@stump/components'
import { Suspense, useState, useTransition } from 'react'

import { useTranslate } from '@/hooks/useTranslate'

import { formatWhen } from './format'
import { useAccessAudit } from './graphql'
import { accessKey } from './keys'
import Pager from './Pager'
import Section, { Empty, Row, RowList } from './Section'

/** The actions the page's own log can hold, named for the keys of the locale files */
const ACTIONS: Record<string, string> = {
	'invite.create': 'inviteCreate',
	'invite.revoke': 'inviteRevoke',
	'invite.redeemed': 'inviteRedeemed',
	'device.revoke': 'deviceRevoke',
	'device.restore': 'deviceRestore',
	'device.reset_budget': 'deviceResetBudget',
	'device.registered_legacy': 'deviceRegisteredLegacy',
	'person.revoke_all': 'personRevokeAll',
	'user.lock': 'userLock',
	'user.locked_by_failures': 'userLockedByFailures',
	'user.unblock': 'userUnblock',
	'user.password_set': 'userPasswordSet',
	'user.password_changed': 'userPasswordChanged',
	'user.sessions_deleted': 'userSessionsDeleted',
	'login_activity.cleared': 'loginActivityCleared',
	'signup.link_created': 'signupLinkCreated',
	'signup.link_revoked': 'signupLinkRevoked',
	'signup.created': 'signupCreated',
	'person.access_set': 'personAccessSet',
	'signin.device_limit': 'signinDeviceLimit',
	'signin.backoff': 'signinBackoff',
	'signin.global_budget': 'signinGlobalBudget',
	'signin.unknown_names': 'signinUnknownNames',
	'device.removed_by_person': 'deviceRemovedByPerson',
	'opds.key_created': 'opdsKeyCreated',
	'opds.key_replaced': 'opdsKeyReplaced',
}

function ActivityList() {
	const { t } = useTranslate()
	const [page, setPage] = useState(1)
	// the page that is shown stays until the next one is ready
	const [isPaging, startTransition] = useTransition()
	const { nodes, pageInfo } = useAccessAudit(page)

	return (
		<div className={cn('gap-y-3 flex flex-col', { 'opacity-60': isPaging })} aria-busy={isPaging}>
			<RowList data-testid="access-activity-list">
				{nodes.length === 0 && <Empty>{t(accessKey('activity.empty'))}</Empty>}
				{nodes.map((entry) => {
					const known = ACTIONS[entry.action]
					return (
						<Row key={entry.id} data-testid={`access-activity-${entry.id}`}>
							<div className="min-w-0">
								{/* an action this page doesn't know is shown as the server wrote it */}
								<Text size="sm" className="font-medium">
									{known ? t(accessKey(`activity.actions.${known}`)) : entry.action}
								</Text>
								{entry.detail && (
									<Text size="xs" variant="muted" className="mt-1 break-words">
										{entry.detail}
									</Text>
								)}
							</div>
							<Text size="xs" variant="muted" className="shrink-0">
								{t(accessKey('activity.when'), {
									actor: entry.actor,
									at: formatWhen(entry.at),
								})}
							</Text>
						</Row>
					)
				})}
			</RowList>
			<Pager
				page={page}
				totalPages={pageInfo.totalPages}
				onChange={(next) => startTransition(() => setPage(next))}
			/>
		</div>
	)
}

/** What was done on this page, and by whom: nothing in it is a token, a link or a password */
export default function ActivitySection() {
	const { t } = useTranslate()

	return (
		<Section
			id="activity"
			title={t(accessKey('activity.title'))}
			description={t(accessKey('activity.description'))}
		>
			<Suspense fallback={null}>
				<ActivityList />
			</Suspense>
		</Section>
	)
}
