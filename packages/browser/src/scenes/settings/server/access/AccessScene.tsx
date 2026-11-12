import { Suspense, useCallback, useState } from 'react'
import { Helmet } from 'react-helmet'
import { useSearchParams } from 'react-router'

import { SceneContainer } from '@/components/container'
import { useTranslate } from '@/hooks/useTranslate'

import AttentionSection, { type InvitePrefill } from './AttentionSection'
import type { AccessPersonRow } from './graphql'
import HistorySection from './HistorySection'
import InviteDialog from './InviteDialog'
import { accessKey } from './keys'
import LetInDialog from './LetInDialog'
import PeopleSection from './PeopleSection'
import PersonSheet from './PersonSheet'
import SetPasswordDialog from './SetPasswordDialog'
import StatusLine from './StatusLine'

/** The address asks for a person's sheet with `?person=` */
const PERSON_PARAM = 'person'

/**
 * *Public access*: who can come in from the internet, and is anything wrong. Four things, in the
 * order the owner needs them: one line on whether it works, what needs the owner's attention
 * (each with its one button, and a way to clear it), the people (with the one big button, *Invite
 * someone*), and the history, collapsed. Everything about a person is in their sheet. It is the
 * owner's alone: the route is only there for the server owner, and the server refuses everything
 * on it to anyone else and to any request that came through the public address
 */
export default function AccessScene() {
	const { t } = useTranslate()
	const [params, setParams] = useSearchParams()
	const personId = params.get(PERSON_PARAM)

	const [invite, setInvite] = useState<{ open: boolean; prefill: InvitePrefill | null }>({
		open: false,
		prefill: null,
	})
	const [letIn, setLetIn] = useState<AccessPersonRow | null>(null)
	const [passwordFor, setPasswordFor] = useState<{ id: string; username: string } | null>(null)
	const [focus, setFocus] = useState<{ failuresOnly: boolean; nonce: number } | null>(null)

	const openPerson = useCallback(
		(id: string) =>
			setParams(
				(current) => {
					const next = new URLSearchParams(current)
					next.set(PERSON_PARAM, id)
					return next
				},
				{ replace: true },
			),
		[setParams],
	)
	const closePerson = useCallback(
		() =>
			setParams(
				(current) => {
					const next = new URLSearchParams(current)
					next.delete(PERSON_PARAM)
					return next
				},
				{ replace: true },
			),
		[setParams],
	)

	return (
		<SceneContainer variant="settings">
			<Helmet>
				<title>Stump | {t(accessKey('helmet'))}</title>
			</Helmet>

			<div className="gap-y-8 flex flex-col">
				<Suspense fallback={null}>
					<StatusLine />
				</Suspense>
				<Suspense fallback={null}>
					<AttentionSection
						onOpenPerson={openPerson}
						onInvite={(prefill) => setInvite({ open: true, prefill })}
						onSeeAttempts={() => setFocus({ failuresOnly: true, nonce: Date.now() })}
						onLetIn={setLetIn}
						onSetPassword={setPasswordFor}
					/>
				</Suspense>
				<Suspense fallback={null}>
					<PeopleSection
						onInvite={() => setInvite({ open: true, prefill: null })}
						onOpenPerson={openPerson}
					/>
				</Suspense>
				<HistorySection focus={focus} />
			</div>

			<Suspense fallback={null}>
				<PersonSheet
					personId={personId}
					onClose={closePerson}
					onLetIn={setLetIn}
					onSetPassword={setPasswordFor}
					hidden={!!letIn || !!passwordFor}
				/>
				<InviteDialog
					open={invite.open}
					prefill={invite.prefill}
					onClose={() => setInvite({ open: false, prefill: null })}
				/>
				<LetInDialog person={letIn} onClose={() => setLetIn(null)} onSetPassword={setPasswordFor} />
			</Suspense>
			{passwordFor && (
				<SetPasswordDialog person={passwordFor} isOpen onClose={() => setPasswordFor(null)} />
			)}
		</SceneContainer>
	)
}
