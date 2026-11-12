import { Badge, Button, DropdownMenu, NewCard, Text } from '@stump/components'
import { SignupLinkKind } from '@stump/graphql'
import { ChevronRight, MoreHorizontal, UserPlus } from 'lucide-react'
import { useState } from 'react'

import { useTranslate } from '@/hooks/useTranslate'

import EndedList, { useEndedCount } from './EndedList'
import { formatAgo, formatWhen } from './format'
import {
	type AccessInvite,
	type AccessPersonRow,
	type AccessSignupLink,
	useAccessInvites,
	useAccessOverview,
	useAccessPeople,
	useAccessSignupLinks,
} from './graphql'
import { accessKey } from './keys'
import RevokeLinkDialog, { type Revoking } from './RevokeLinkDialog'

type Props = {
	onInvite: () => void
	onOpenPerson: (personId: string) => void
}

/** The newest sign-in first; people who never signed in last */
const byLastSignIn = (a: AccessPersonRow, b: AccessPersonRow) =>
	new Date(b.lastSignInAt ?? 0).getTime() - new Date(a.lastSignInAt ?? 0).getTime()

/** A person's first letter in a circle, since the page has no photo of them */
function Initial({ name }: { name: string }) {
	return (
		<span
			aria-hidden
			className="h-8 w-8 text-sm font-medium flex shrink-0 items-center justify-center rounded-full bg-muted uppercase"
		>
			{name.slice(0, 1)}
		</span>
	)
}

/** One person: who they are, how they stand, and a click opens everything about them */
function PersonRow({ row, onOpen }: { row: AccessPersonRow; onOpen: () => void }) {
	const { t } = useTranslate()
	const { person } = row

	const line = row.lastSignInAt
		? t(accessKey('people.devicesLine'), {
				active: row.activeDevices,
				limit: row.maxDevices,
				when: formatAgo(row.lastSignInAt),
			})
		: t(accessKey('people.devicesLineNever'), {
				active: row.activeDevices,
				limit: row.maxDevices,
			})

	return (
		<button
			type="button"
			data-testid={`access-person-${person.id}`}
			onClick={onOpen}
			title={row.lastSignInAt ? formatWhen(row.lastSignInAt) : undefined}
			className="gap-3 px-4 py-3 flex w-full items-center text-left hover:bg-muted/40 focus-visible:bg-muted/40 focus-visible:outline-none"
		>
			<Initial name={person.username} />
			<span className="min-w-0 flex-1">
				<span className="gap-2 flex flex-wrap items-center">
					<Text size="sm" className="font-medium">
						{person.username}
					</Text>
					{person.locked && (
						<Badge size="xs" variant="error">
							{t(accessKey('people.badges.locked'))}
						</Badge>
					)}
					{row.beyondFriendSet.length > 0 && (
						<Badge size="xs" variant="warning">
							{t(accessKey('people.badges.extras'))}
						</Badge>
					)}
					{row.mustChangePassword && (
						<Badge size="xs" variant="secondary">
							{t(accessKey('people.badges.mustChange'))}
						</Badge>
					)}
				</span>
				<Text size="xs" variant="muted">
					{row.publicSignin ? line : t(accessKey('people.homeOnly'))}
				</Text>
			</span>
			<ChevronRight aria-hidden className="h-4 w-4 shrink-0 text-muted-foreground" />
		</button>
	)
}

/** A link that was made and hasn't been used up: its note, what it is and when it ends */
function InvitedRow({
	title,
	line,
	onRevoke,
	testId,
}: {
	title: string
	line: string
	onRevoke: () => void
	testId: string
}) {
	const { t } = useTranslate()
	return (
		<div data-testid={testId} className="gap-3 px-4 py-3 flex items-center">
			<span className="min-w-0 flex-1">
				<Text size="sm" className="font-medium">
					{title}
				</Text>
				<Text size="xs" variant="muted">
					{line}
				</Text>
			</span>
			<DropdownMenu
				align="end"
				trigger={
					<Button
						variant="ghost"
						size="icon"
						aria-label={t(accessKey('people.moreActions'))}
						className="shrink-0"
					>
						<MoreHorizontal className="h-4 w-4" />
					</Button>
				}
				groups={[
					{
						items: [
							{ label: t(accessKey('people.revoke')), variant: 'destructive', onClick: onRevoke },
						],
					},
				]}
			/>
		</div>
	)
}

const signupLine = (t: ReturnType<typeof useTranslate>['t'], link: AccessSignupLink) =>
	link.kind === SignupLinkKind.Single
		? t(accessKey('people.invitedOne'), { when: formatAgo(link.expiresAt) })
		: t(accessKey('people.invitedOpen'), {
				used: link.uses,
				max: link.maxUses,
				when: formatAgo(link.expiresAt),
			})

const deviceLinkLine = (t: ReturnType<typeof useTranslate>['t'], invite: AccessInvite) =>
	t(accessKey('people.invitedDevice'), {
		name: invite.person?.username ?? '',
		when: formatAgo(invite.expiresAt),
	})

/**
 * Who can sign in from the internet, in one list: the people who can, the people who were invited
 * and haven't come in yet, and (behind a click) the people who can't. The one big button of the
 * page is *Invite someone*. A click on a person opens everything about them; nothing here has a
 * control of its own, so the list reads at a glance and works the same on a phone
 */
export default function PeopleSection({ onInvite, onOpenPerson }: Props) {
	const { t } = useTranslate()
	const overview = useAccessOverview()
	const people = useAccessPeople()
	const links = useAccessSignupLinks(false).nodes
	const invites = useAccessInvites(false).nodes
	const ended = useEndedCount()

	const [showHome, setShowHome] = useState(false)
	const [showEnded, setShowEnded] = useState(false)
	const [revoking, setRevoking] = useState<Revoking | null>(null)

	// the server owner never signs in from the internet, and a removed account is gone
	const others = people.filter(({ person }) => !person.isServerOwner && !person.removed)
	const inside = others.filter((row) => row.publicSignin).sort(byLastSignIn)
	const home = others.filter((row) => !row.publicSignin).sort(byLastSignIn)
	const invited = links.length + invites.length

	// soonest to end first
	const invitedRows = [
		...links.map((link) => ({ at: link.expiresAt, link })),
		...invites.map((invite) => ({ at: invite.expiresAt, invite })),
	].sort((a, b) => new Date(a.at).getTime() - new Date(b.at).getTime())

	return (
		<section
			id="access-people"
			data-testid="access-people"
			aria-labelledby="access-people-title"
			className="gap-3 flex flex-col"
		>
			<NewCard
				label={t(accessKey('people.title'))}
				description={t(accessKey('people.description'))}
				actions={
					<Button
						onClick={onInvite}
						disabled={!overview.configured}
						title={overview.configured ? undefined : t(accessKey('people.inviteNeedsSetup'))}
						className="max-sm:w-full"
					>
						<UserPlus className="h-4 w-4" />
						{t(accessKey('people.invite'))}
					</Button>
				}
			>
				{!overview.configured && (
					<Text
						size="sm"
						variant="muted"
						className="px-4 pt-3"
						data-testid="access-invite-needs-setup"
					>
						{t(accessKey('people.inviteNeedsSetup'))}
					</Text>
				)}
				{inside.length === 0 && invited === 0 && home.length === 0 && ended === 0 ? (
					<Text size="sm" variant="muted" className="px-4 py-6 text-center">
						{t(accessKey('people.empty'))}
					</Text>
				) : (
					<>
						{inside.length > 0 && (
							<div data-testid="access-people-internet">
								<Text size="xs" variant="muted" className="px-4 pt-3 pb-1 font-medium">
									{t(accessKey('people.groups.internet'), { count: inside.length })}
								</Text>
								<div className="divide-y divide-border/70">
									{inside.map((row) => (
										<PersonRow
											key={row.person.id}
											row={row}
											onOpen={() => onOpenPerson(row.person.id)}
										/>
									))}
								</div>
							</div>
						)}
						{inside.length === 0 && (
							<Text size="sm" variant="muted" className="px-4 py-4">
								{t(accessKey('people.nobody'))}
							</Text>
						)}

						{invited > 0 && (
							<div data-testid="access-people-invited" className="border-t border-border/70">
								<Text size="xs" variant="muted" className="px-4 pt-3 pb-1 font-medium">
									{t(accessKey('people.groups.invited'), { count: invited })}
								</Text>
								<div className="divide-y divide-border/70">
									{invitedRows.map((entry) =>
										'link' in entry ? (
											<InvitedRow
												key={entry.link.id}
												testId={`access-invited-${entry.link.id}`}
												title={entry.link.label}
												line={signupLine(t, entry.link)}
												onRevoke={() =>
													setRevoking({
														kind: 'signup',
														id: entry.link.id,
														label: entry.link.label,
														accounts: entry.link.people.length,
													})
												}
											/>
										) : (
											<InvitedRow
												key={entry.invite.id}
												testId={`access-invited-${entry.invite.id}`}
												title={entry.invite.label}
												line={deviceLinkLine(t, entry.invite)}
												onRevoke={() =>
													setRevoking({
														kind: 'device',
														id: entry.invite.id,
														label: entry.invite.label,
													})
												}
											/>
										),
									)}
								</div>
							</div>
						)}

						{home.length > 0 && (
							<div data-testid="access-people-home" className="border-t border-border/70">
								<Button
									variant="ghost"
									size="sm"
									className="m-2"
									aria-expanded={showHome}
									onClick={() => setShowHome((open) => !open)}
								>
									{t(accessKey('people.groups.home'), { count: home.length })}
									<ChevronRight
										aria-hidden
										className={showHome ? 'h-4 w-4 rotate-90' : 'h-4 w-4'}
									/>
								</Button>
								{showHome && (
									<div className="divide-y divide-border/70">
										{home.map((row) => (
											<PersonRow
												key={row.person.id}
												row={row}
												onOpen={() => onOpenPerson(row.person.id)}
											/>
										))}
									</div>
								)}
							</div>
						)}

						{ended > 0 && (
							<div className="border-t border-border/70">
								<Button
									variant="ghost"
									size="sm"
									className="m-2"
									aria-expanded={showEnded}
									onClick={() => setShowEnded((open) => !open)}
									data-testid="access-show-ended"
								>
									{showEnded
										? t(accessKey('people.hideEnded'))
										: t(accessKey('people.showEnded'), { count: ended })}
								</Button>
								{showEnded && <EndedList />}
							</div>
						)}
					</>
				)}
			</NewCard>

			<RevokeLinkDialog link={revoking} onClose={() => setRevoking(null)} />
		</section>
	)
}
