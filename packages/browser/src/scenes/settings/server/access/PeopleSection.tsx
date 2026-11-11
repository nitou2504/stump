import { Badge, Button, ButtonOrLink, Text } from '@stump/components'
import { Suspense, useState } from 'react'
import { toast } from 'sonner'

import { useTranslate } from '@/hooks/useTranslate'
import { usePaths } from '@/paths'

import { formatWhen } from './format'
import { type AccessPersonRow, useAccessActions, useAccessPeople } from './graphql'
import { accessKey } from './keys'
import Section, { Empty, Row, RowList } from './Section'
import SetPasswordDialog from './SetPasswordDialog'

const errorMessage = (error: unknown) =>
	error instanceof Error && error.message ? error.message : undefined

function PeopleList() {
	const { t } = useTranslate()
	const paths = usePaths()
	const people = useAccessPeople()
	const actions = useAccessActions()
	const [settingFor, setSettingFor] = useState<AccessPersonRow['person'] | null>(null)

	const unblock = async (id: string, name: string) => {
		try {
			await actions.unblock(id)
			toast.success(t(accessKey('signIns.unblocked'), { name }))
		} catch (error) {
			toast.error(t(accessKey('signIns.errors.unblock')), { description: errorMessage(error) })
		}
	}

	return (
		<>
			<RowList data-testid="access-people-list">
				{people.length === 0 && <Empty>{t(accessKey('people.empty'))}</Empty>}
				{people.map(({ person, lastSignInAt, mustChangePassword }) => (
					<Row key={person.id} data-testid={`access-people-${person.id}`}>
						<div className="min-w-0">
							<div className="gap-2 flex flex-wrap items-center">
								<Text size="sm" className="font-medium">
									{person.username}
								</Text>
								{person.isServerOwner && (
									<Badge size="xs" variant="primary">
										{t(accessKey('people.owner'))}
									</Badge>
								)}
								{person.locked && (
									<Badge size="xs" variant="warning">
										{t(accessKey('people.locked'))}
									</Badge>
								)}
								{mustChangePassword && (
									<Badge size="xs" variant="secondary">
										{t(accessKey('people.mustChange'))}
									</Badge>
								)}
							</div>
							<Text size="xs" variant="muted" className="mt-1">
								{t(accessKey('people.lastSignIn'), {
									at: lastSignInAt ? formatWhen(lastSignInAt) : t(accessKey('people.never')),
								})}
							</Text>
						</div>
						<div className="gap-2 flex flex-wrap">
							{person.locked && (
								<Button
									size="sm"
									variant="outline"
									disabled={actions.isPending}
									onClick={() => unblock(person.id, person.username)}
								>
									{t(accessKey('signIns.unblock'))}
								</Button>
							)}
							{/* the owner's password is not set from here, and it is never used from the internet */}
							{!person.isServerOwner && !person.removed && (
								<Button size="sm" variant="outline" onClick={() => setSettingFor(person)}>
									{t(accessKey('people.setPassword'))}
								</Button>
							)}
							{!person.removed && (
								<ButtonOrLink size="sm" variant="ghost" href={paths.updateUser(person.id)}>
									{t(accessKey('people.manage'))}
								</ButtonOrLink>
							)}
						</div>
					</Row>
				))}
			</RowList>

			{settingFor && (
				<SetPasswordDialog
					person={settingFor}
					isOpen={!!settingFor}
					onClose={() => setSettingFor(null)}
				/>
			)}
		</>
	)
}

/** The accounts, for the two things the owner needs for somebody who can't get in: a new password, or the way in again */
export default function PeopleSection() {
	const { t } = useTranslate()

	return (
		<Section
			id="people"
			title={t(accessKey('people.title'))}
			description={t(accessKey('people.description'))}
		>
			<Suspense fallback={null}>
				<PeopleList />
			</Suspense>
		</Section>
	)
}
