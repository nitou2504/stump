import { Badge, Button, cn, NewCard, Switch, Tabs, Text } from '@stump/components'
import { AccessSignInReason } from '@stump/graphql'
import { ChevronDown } from 'lucide-react'
import { Suspense, useEffect, useRef, useState, useTransition } from 'react'

import { useTranslate } from '@/hooks/useTranslate'

import { formatAgo, formatWhen } from './format'
import {
	type AccessAuditEntry,
	type AccessSignIn,
	useAccessAudit,
	useAccessSignIns,
} from './graphql'
import { accessKey } from './keys'
import Pager from './Pager'
import PersonLabel from './PersonLabel'

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
	'warning.dismiss': 'warningDismiss',
	'warning.restore': 'warningRestore',
}

/** How a sign-in ended, in the words of the locale files */
const RESULT_KEY: Record<AccessSignInReason, string> = {
	[AccessSignInReason.WrongPassword]: 'wrongPassword',
	[AccessSignInReason.Locked]: 'locked',
	[AccessSignInReason.OwnerViaEdge]: 'ownerViaEdge',
	[AccessSignInReason.WrongPerson]: 'wrongPerson',
	[AccessSignInReason.NotAllowedPublicly]: 'notAllowed',
	[AccessSignInReason.DeviceLimit]: 'deviceLimit',
}

/** What opens a row's second line: the part nobody needs until they ask */
function Expandable({
	children,
	detail,
	testId,
}: {
	children: React.ReactNode
	detail: React.ReactNode
	testId: string
}) {
	const [open, setOpen] = useState(false)
	return (
		<div data-testid={testId} className="px-4 py-3">
			<button
				type="button"
				aria-expanded={open}
				onClick={() => setOpen((next) => !next)}
				className="gap-3 flex w-full items-start justify-between text-left"
			>
				<span className="min-w-0 flex-1">{children}</span>
				<ChevronDown aria-hidden className={cn('mt-1 h-4 w-4 shrink-0', { 'rotate-180': open })} />
			</button>
			{open && (
				<Text
					size="xs"
					variant="muted"
					className="mt-2 break-words"
					data-testid={`${testId}-detail`}
				>
					{detail}
				</Text>
			)}
		</div>
	)
}

function SignInRow({ entry }: { entry: AccessSignIn }) {
	const { t } = useTranslate()
	const result = entry.successful ? 'ok' : entry.reason ? RESULT_KEY[entry.reason] : 'failed'

	return (
		<Expandable
			testId={`access-signin-${entry.id}`}
			detail={`${entry.ip || '–'} · ${entry.userAgent || '–'}`}
		>
			<span className="gap-2 flex flex-wrap items-center">
				<PersonLabel person={entry.person} />
				<Badge size="xs" variant={entry.successful ? 'success' : 'error'}>
					{t(accessKey(`history.result.${result}`))}
				</Badge>
			</span>
			<Text size="xs" variant="muted" title={formatWhen(entry.at)}>
				{formatAgo(entry.at)} · {entry.deviceLabel ?? entry.agentLabel}
			</Text>
		</Expandable>
	)
}

function SignIns({ failuresOnly }: { failuresOnly: boolean }) {
	const { t } = useTranslate()
	const [page, setPage] = useState(1)
	// the page that is shown stays until the next one is ready
	const [isPaging, startTransition] = useTransition()
	const { nodes, pageInfo } = useAccessSignIns(failuresOnly, page)

	return (
		<div className={cn('gap-y-3 flex flex-col', { 'opacity-60': isPaging })} aria-busy={isPaging}>
			<div className="divide-y divide-border/70" data-testid="access-signins-list">
				{nodes.length === 0 && (
					<Text size="sm" variant="muted" className="px-4 py-6 text-center">
						{t(accessKey('history.noSignIns'))}
					</Text>
				)}
				{nodes.map((entry) => (
					<SignInRow key={entry.id} entry={entry} />
				))}
			</div>
			<Pager
				page={page}
				totalPages={pageInfo.totalPages}
				onChange={(next) => startTransition(() => setPage(next))}
			/>
		</div>
	)
}

function ChangeRow({ entry }: { entry: AccessAuditEntry }) {
	const { t } = useTranslate()
	const known = ACTIONS[entry.action]
	// an action this page doesn't know is shown as the server wrote it
	const label = known ? t(accessKey(`activity.actions.${known}`)) : entry.action
	const who =
		entry.actor === 'public'
			? t(accessKey('history.fromInternet'))
			: entry.actor === 'system'
				? t(accessKey('history.system'))
				: entry.actor

	return (
		<Expandable testId={`access-activity-${entry.id}`} detail={entry.detail || '–'}>
			<Text size="sm" className="font-medium">
				{entry.targetLabel ? `${label}: ${entry.targetLabel}` : label}
			</Text>
			<Text size="xs" variant="muted" title={formatWhen(entry.at)}>
				{who} · {formatAgo(entry.at)}
			</Text>
		</Expandable>
	)
}

function Changes() {
	const { t } = useTranslate()
	const [page, setPage] = useState(1)
	const [isPaging, startTransition] = useTransition()
	const { nodes, pageInfo } = useAccessAudit(page)

	return (
		<div className={cn('gap-y-3 flex flex-col', { 'opacity-60': isPaging })} aria-busy={isPaging}>
			<div className="divide-y divide-border/70" data-testid="access-activity-list">
				{nodes.length === 0 && (
					<Text size="sm" variant="muted" className="px-4 py-6 text-center">
						{t(accessKey('history.noChanges'))}
					</Text>
				)}
				{nodes.map((entry) => (
					<ChangeRow key={entry.id} entry={entry} />
				))}
			</div>
			<Pager
				page={page}
				totalPages={pageInfo.totalPages}
				onChange={(next) => startTransition(() => setPage(next))}
			/>
		</div>
	)
}

type Props = {
	/** Asks the history to open on the failed sign-ins (a new value each time) */
	focus?: { failuresOnly: boolean; nonce: number } | null
}

/**
 * What happened: the other people's sign-ins (the owner's own are in Users → Authentication
 * history) and what the owner changed here. Collapsed, because it is for the day something is
 * wrong; nothing the rows hold at first is technical, and the address and the browser are one
 * click away
 */
export default function HistorySection({ focus }: Props) {
	const { t } = useTranslate()
	const [open, setOpen] = useState(false)
	const [tab, setTab] = useState('signins')
	const [failuresOnly, setFailuresOnly] = useState(false)
	const [, startTransition] = useTransition()
	const section = useRef<HTMLElement>(null)

	// a warning that points at the attempts opens the history on them, and takes you there (it is
	// the last block of the page: on its own the click would seem to do nothing)
	useEffect(() => {
		if (!focus) return
		setOpen(true)
		setTab('signins')
		setFailuresOnly(focus.failuresOnly)
		// once the list has come in under the heading: before, the page is too short to bring the
		// heading to the top
		const timer = window.setTimeout(
			() => section.current?.scrollIntoView?.({ block: 'start', behavior: 'smooth' }),
			300,
		)
		return () => window.clearTimeout(timer)
	}, [focus])

	return (
		<section
			ref={section}
			id="access-history"
			data-testid="access-history"
			className="gap-3 flex flex-col"
		>
			<NewCard label={t(accessKey('history.title'))}>
				{!open ? (
					<div className="px-4 py-3">
						<Button variant="ghost" size="sm" onClick={() => setOpen(true)}>
							{t(accessKey('history.show'))}
						</Button>
					</div>
				) : (
					<div className="gap-3 py-3 flex flex-col">
						<div className="gap-3 px-4 flex flex-wrap items-center justify-between">
							<Tabs value={tab} onValueChange={setTab} variant="primary">
								<Tabs.List>
									<Tabs.Trigger value="signins">{t(accessKey('history.signIns'))}</Tabs.Trigger>
									<Tabs.Trigger value="changes">{t(accessKey('history.changes'))}</Tabs.Trigger>
								</Tabs.List>
							</Tabs>
							{tab === 'signins' && (
								<Switch
									id="access-failures-only"
									label={t(accessKey('history.failuresOnly'))}
									checked={failuresOnly}
									onCheckedChange={(checked) => startTransition(() => setFailuresOnly(checked))}
								/>
							)}
						</div>
						{/* a new filter starts at the first page */}
						<Suspense fallback={null}>
							{tab === 'signins' ? (
								<SignIns key={String(failuresOnly)} failuresOnly={failuresOnly} />
							) : (
								<Changes />
							)}
						</Suspense>
						{tab === 'signins' && (
							<Text size="xs" variant="muted" className="px-4">
								{t(accessKey('history.allSignIns'), {
									users: t('settingsScene.server/users.title'),
								})}
							</Text>
						)}
						<div className="px-4">
							<Button variant="ghost" size="sm" onClick={() => setOpen(false)}>
								{t(accessKey('history.hide'))}
							</Button>
						</div>
					</div>
				)}
			</NewCard>
		</section>
	)
}
