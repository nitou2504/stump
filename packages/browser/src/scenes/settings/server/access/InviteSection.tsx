import { Badge, Button, Card, cn, Input, NativeSelect, Text } from '@stump/components'
import { AccessInviteStatus } from '@stump/graphql'
import { Check, Copy } from 'lucide-react'
import { FormEvent, Suspense, useMemo, useState, useTransition } from 'react'
import { toast } from 'sonner'

import { useTranslate } from '@/hooks/useTranslate'

import { copyText } from './copy'
import { formatWhen } from './format'
import {
	type AccessInvite,
	INVITE_HOURS,
	INVITE_USES,
	useAccessActions,
	useAccessInvites,
	useAccessOverview,
	useAccessPeople,
} from './graphql'
import { accessKey } from './keys'
import Pager from './Pager'
import PersonLabel from './PersonLabel'
import Section, { Empty, Row, RowList } from './Section'

const errorMessage = (error: unknown) =>
	error instanceof Error && error.message ? error.message : undefined

/** What the owner is shown right after making an invite: the one and only copy of its link */
type MadeLink = { link: string; person: string; label: string }

/**
 * A link that was just made, with a button to copy it. It is shown here and nowhere else: Stump
 * keeps a hash of the token and can't show the link again, so a lost one means making another
 */
function MadeLinkBox({ made, onDismiss }: { made: MadeLink; onDismiss: () => void }) {
	const { t } = useTranslate()
	const [copied, setCopied] = useState(false)

	const copy = async () => {
		const done = await copyText(made.link)
		setCopied(done)
		if (done) {
			toast.success(t(accessKey('invites.created.copied')))
		} else {
			// nothing could be copied: the link is selected in the box, to copy by hand
			toast.error(t(accessKey('invites.created.copyFailed')))
		}
	}

	return (
		<Card data-testid="access-made-link" className="gap-3 p-4 flex flex-col" role="status">
			<Text className="font-medium">
				{t(accessKey('invites.created.title'), { person: made.person, label: made.label })}
			</Text>
			<div className="gap-2 sm:flex-row flex flex-col">
				<Input
					aria-label={t(accessKey('invites.created.linkLabel'))}
					value={made.link}
					readOnly
					fullWidth
					onFocus={(event) => event.currentTarget.select()}
				/>
				<Button onClick={copy} className="shrink-0">
					{copied ? <Check className="h-4 w-4" /> : <Copy className="h-4 w-4" />}
					{t(accessKey('invites.created.copy'))}
				</Button>
			</div>
			<Text size="sm" variant="muted">
				{t(accessKey('invites.created.once'))}
			</Text>
			<div>
				<Button size="sm" variant="ghost" onClick={onDismiss}>
					{t(accessKey('invites.created.dismiss'))}
				</Button>
			</div>
		</Card>
	)
}

function InviteForm({ onMade }: { onMade: (made: MadeLink) => void }) {
	const { t } = useTranslate()
	const overview = useAccessOverview()
	const people = useAccessPeople()
	const actions = useAccessActions()

	const [personId, setPersonId] = useState('')
	const [label, setLabel] = useState('')
	const [hours, setHours] = useState<number>(72)
	const [uses, setUses] = useState<number>(3)

	// the owner never signs in from the internet, and an account that is removed or locked can't
	// use an invite: the server refuses them, so they aren't offered
	const eligible = useMemo(
		() => people.filter(({ person }) => !person.isServerOwner && !person.removed && !person.locked),
		[people],
	)

	const canSubmit = overview.configured && !!personId && !!label.trim() && !actions.isPending

	const submit = async (event: FormEvent) => {
		event.preventDefault()
		if (!canSubmit) return
		const person = eligible.find(({ person }) => person.id === personId)?.person.username ?? ''
		try {
			const link = await actions.createInvite({
				personId,
				label: label.trim(),
				validForHours: hours,
				maxUses: uses,
			})
			onMade({ link, person, label: label.trim() })
			setLabel('')
		} catch (error) {
			toast.error(t(accessKey('invites.errors.create')), { description: errorMessage(error) })
		}
	}

	return (
		<Card className="p-4">
			<form
				onSubmit={submit}
				className="gap-4 md:grid-cols-2 xl:grid-cols-4 grid grid-cols-1 items-end"
			>
				<div className="gap-1.5 flex flex-col">
					<label htmlFor="access-invite-person" className="text-sm font-medium">
						{t(accessKey('invites.form.person'))}
					</label>
					<NativeSelect
						id="access-invite-person"
						value={personId}
						onChange={(event) => setPersonId(event.target.value)}
						options={eligible.map(({ person }) => ({ label: person.username, value: person.id }))}
						emptyOption={{ label: t(accessKey('invites.form.choosePerson')), value: '' }}
					/>
				</div>
				<Input
					id="access-invite-label"
					label={t(accessKey('invites.form.label'))}
					placeholder={t(accessKey('invites.form.labelPlaceholder'))}
					value={label}
					onChange={(event) => setLabel(event.target.value)}
					maxLength={40}
					fullWidth
				/>
				<div className="gap-1.5 flex flex-col">
					<label htmlFor="access-invite-hours" className="text-sm font-medium">
						{t(accessKey('invites.form.validFor'))}
					</label>
					<NativeSelect
						id="access-invite-hours"
						value={hours}
						onChange={(event) => setHours(Number(event.target.value))}
						options={INVITE_HOURS.map((value) => ({
							label: t(accessKey(`invites.validity.h${value}`)),
							value,
						}))}
					/>
				</div>
				<div className="gap-1.5 flex flex-col">
					<label htmlFor="access-invite-uses" className="text-sm font-medium">
						{t(accessKey('invites.form.uses'))}
					</label>
					<NativeSelect
						id="access-invite-uses"
						value={uses}
						onChange={(event) => setUses(Number(event.target.value))}
						options={INVITE_USES.map((value) => ({ label: String(value), value }))}
					/>
				</div>
				<div className="md:col-span-2 xl:col-span-4">
					<Button type="submit" disabled={!canSubmit} isLoading={actions.isPending}>
						{t(accessKey('invites.form.create'))}
					</Button>
					{!eligible.length && (
						<Text size="sm" variant="muted" className="mt-2">
							{t(accessKey('invites.form.noPeople'))}
						</Text>
					)}
				</div>
			</form>
		</Card>
	)
}

const STATUS_BADGE: Record<AccessInviteStatus, 'success' | 'primary' | 'secondary' | 'warning'> = {
	[AccessInviteStatus.Unused]: 'success',
	[AccessInviteStatus.Used]: 'primary',
	[AccessInviteStatus.UsedUp]: 'secondary',
	[AccessInviteStatus.Expired]: 'secondary',
	[AccessInviteStatus.Revoked]: 'warning',
}

const STATUS_KEY: Record<AccessInviteStatus, string> = {
	[AccessInviteStatus.Unused]: 'unused',
	[AccessInviteStatus.Used]: 'used',
	[AccessInviteStatus.UsedUp]: 'usedUp',
	[AccessInviteStatus.Expired]: 'expired',
	[AccessInviteStatus.Revoked]: 'revoked',
}

/** An invite that can still be opened can still be taken back */
const isOpen = (invite: AccessInvite) =>
	invite.status === AccessInviteStatus.Unused || invite.status === AccessInviteStatus.Used

function InviteList() {
	const { t } = useTranslate()
	const [page, setPage] = useState(1)
	// the page that is shown stays until the next one is ready
	const [isPaging, startTransition] = useTransition()
	const { nodes, pageInfo } = useAccessInvites(page)
	const actions = useAccessActions()

	const revoke = async (invite: AccessInvite) => {
		try {
			await actions.revokeInvite(invite.id)
			toast.success(t(accessKey('invites.revoked'), { label: invite.label }))
		} catch (error) {
			toast.error(t(accessKey('invites.errors.revoke')), { description: errorMessage(error) })
		}
	}

	return (
		<div className={cn('gap-y-3 flex flex-col', { 'opacity-60': isPaging })} aria-busy={isPaging}>
			<RowList data-testid="access-invite-list">
				{nodes.length === 0 && <Empty>{t(accessKey('invites.empty'))}</Empty>}
				{nodes.map((invite) => (
					<Row key={invite.id} data-testid={`access-invite-${invite.id}`}>
						<div className="min-w-0">
							<div className="gap-2 flex flex-wrap items-center">
								<PersonLabel person={invite.person} />
								<Text size="sm" variant="muted">
									· {invite.label}
								</Text>
								<Badge size="xs" variant={STATUS_BADGE[invite.status]}>
									{t(accessKey(`invites.status.${STATUS_KEY[invite.status]}`))}
								</Badge>
							</div>
							<Text size="xs" variant="muted" className="mt-1">
								{t(accessKey('invites.detail'), {
									created: formatWhen(invite.createdAt),
									expires: formatWhen(invite.expiresAt),
									uses: invite.uses,
									max: invite.maxUses,
								})}
							</Text>
						</div>
						{isOpen(invite) && (
							<Button
								size="sm"
								variant="outline"
								disabled={actions.isPending}
								onClick={() => revoke(invite)}
							>
								{t(accessKey('invites.revoke'))}
							</Button>
						)}
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

/** Invite links: make one for a person and a device, copy it, and see which are still open */
export default function InviteSection() {
	const { t } = useTranslate()
	const [made, setMade] = useState<MadeLink | null>(null)

	return (
		<Section
			id="invites"
			title={t(accessKey('invites.title'))}
			description={t(accessKey('invites.description'))}
		>
			<Suspense fallback={null}>
				<InviteForm onMade={setMade} />
			</Suspense>
			{made && <MadeLinkBox made={made} onDismiss={() => setMade(null)} />}
			<Suspense fallback={null}>
				<InviteList />
			</Suspense>
		</Section>
	)
}
