import { Badge, Button, cn, Switch, Text } from '@stump/components'
import { AccessLockedBy, AccessSignInReason } from '@stump/graphql'
import { Suspense, useState, useTransition } from 'react'
import { toast } from 'sonner'

import { useTranslate } from '@/hooks/useTranslate'

import { formatWhen, shortAgent } from './format'
import { useAccessActions, useAccessBlocked, useAccessSignIns } from './graphql'
import { accessKey } from './keys'
import Pager from './Pager'
import PersonLabel from './PersonLabel'
import Section, { Empty, Row, RowList } from './Section'

const errorMessage = (error: unknown) =>
	error instanceof Error && error.message ? error.message : undefined

const LOCKED_BY_KEY: Record<AccessLockedBy, string> = {
	[AccessLockedBy.Failures]: 'failures',
	[AccessLockedBy.Owner]: 'owner',
	[AccessLockedBy.Unknown]: 'unknown',
}

const REASON_KEY: Record<AccessSignInReason, string> = {
	[AccessSignInReason.WrongPassword]: 'wrongPassword',
	[AccessSignInReason.Locked]: 'locked',
	[AccessSignInReason.OwnerViaEdge]: 'ownerViaEdge',
	[AccessSignInReason.WrongPerson]: 'wrongPerson',
}

/** The accounts that are locked right now, and the button that lets them in again */
function BlockedList() {
	const { t } = useTranslate()
	const blocked = useAccessBlocked()
	const actions = useAccessActions()

	const unblock = async (id: string, name: string) => {
		try {
			await actions.unblock(id)
			toast.success(t(accessKey('signIns.unblocked'), { name }))
		} catch (error) {
			toast.error(t(accessKey('signIns.errors.unblock')), { description: errorMessage(error) })
		}
	}

	return (
		<div className="gap-y-2 flex flex-col">
			<Text size="sm" className="font-medium">
				{t(accessKey('signIns.blockedTitle'))}
			</Text>
			<RowList data-testid="access-blocked-list">
				{blocked.length === 0 && <Empty>{t(accessKey('signIns.nobodyBlocked'))}</Empty>}
				{blocked.map((entry) => (
					<Row key={entry.person.id} data-testid={`access-blocked-${entry.person.id}`}>
						<div className="min-w-0">
							<div className="gap-2 flex flex-wrap items-center">
								<PersonLabel person={entry.person} />
								<Badge size="xs" variant="warning">
									{t(accessKey(`signIns.lockedBy.${LOCKED_BY_KEY[entry.lockedBy]}`))}
								</Badge>
							</div>
							<Text size="xs" variant="muted" className="mt-1">
								{t(accessKey('signIns.blockedDetail'), {
									count: entry.failuresLast24H,
									last: formatWhen(entry.lastFailureAt),
								})}
							</Text>
						</div>
						<Button
							size="sm"
							variant="outline"
							disabled={actions.isPending}
							onClick={() => unblock(entry.person.id, entry.person.username)}
						>
							{t(accessKey('signIns.unblock'))}
						</Button>
					</Row>
				))}
			</RowList>
			<Text size="xs" variant="muted">
				{t(accessKey('signIns.edgeLimitNote'))}
			</Text>
		</div>
	)
}

/** The sign-in attempts, latest first: when, who, from where, with which device, and what came of it */
function SignInList({ failuresOnly }: { failuresOnly: boolean }) {
	const { t } = useTranslate()
	const [page, setPage] = useState(1)
	// the page that is shown stays until the next one is ready
	const [isPaging, startTransition] = useTransition()
	const { nodes, pageInfo } = useAccessSignIns(failuresOnly, page)

	return (
		<div className={cn('gap-y-3 flex flex-col', { 'opacity-60': isPaging })} aria-busy={isPaging}>
			<RowList data-testid="access-signin-list">
				{nodes.length === 0 && <Empty>{t(accessKey('signIns.empty'))}</Empty>}
				{nodes.map((entry) => (
					<Row key={entry.id} data-testid={`access-signin-${entry.id}`}>
						<div className="min-w-0">
							<div className="gap-2 flex flex-wrap items-center">
								<PersonLabel person={entry.person} />
								<Badge size="xs" variant={entry.successful ? 'success' : 'error'}>
									{t(accessKey(entry.successful ? 'signIns.ok' : 'signIns.failed'))}
								</Badge>
								{entry.reason && (
									<Text size="xs" variant="muted">
										{t(accessKey(`signIns.reason.${REASON_KEY[entry.reason]}`))}
									</Text>
								)}
								{entry.cleared && (
									<Badge size="xs" variant="secondary">
										{t(accessKey('signIns.cleared'))}
									</Badge>
								)}
							</div>
							<Text size="xs" variant="muted" className="mt-1" title={entry.userAgent || undefined}>
								{t(accessKey('signIns.detail'), {
									at: formatWhen(entry.at),
									ip: entry.ip || '–',
									device: entry.deviceLabel ?? entry.deviceId ?? '–',
								})}
								{entry.userAgent ? ` · ${shortAgent(entry.userAgent)}` : ''}
							</Text>
						</div>
					</Row>
				))}
			</RowList>
			<Pager
				page={page}
				totalPages={pageInfo.totalPages}
				onChange={(next) => startTransition(() => setPage(next))}
			/>
		</div>
	)
}

/**
 * Sign-in protection: who is locked out right now (with the way to let them back in), and the
 * attempts that were made. A password is never kept, so none is shown
 */
export default function SignInSection() {
	const { t } = useTranslate()
	const [failuresOnly, setFailuresOnly] = useState(false)
	// the list that is shown stays until the filtered one is ready
	const [, startTransition] = useTransition()

	return (
		<Section
			id="signins"
			title={t(accessKey('signIns.title'))}
			description={t(accessKey('signIns.description'))}
		>
			<Suspense fallback={null}>
				<BlockedList />
			</Suspense>

			<div className="gap-y-2 flex flex-col">
				<div className="gap-3 flex flex-wrap items-center justify-between">
					<Text size="sm" className="font-medium">
						{t(accessKey('signIns.recentTitle'))}
					</Text>
					<Switch
						id="access-failures-only"
						label={t(accessKey('signIns.failuresOnly'))}
						checked={failuresOnly}
						onCheckedChange={(checked) => startTransition(() => setFailuresOnly(checked))}
					/>
				</div>
				{/* a new filter starts at the first page */}
				<Suspense fallback={null}>
					<SignInList key={String(failuresOnly)} failuresOnly={failuresOnly} />
				</Suspense>
			</div>
		</Section>
	)
}
