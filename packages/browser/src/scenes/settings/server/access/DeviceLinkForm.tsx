import { Button, Input, NativeSelect } from '@stump/components'
import { FormEvent, useState } from 'react'
import { toast } from 'sonner'

import { useTranslate } from '@/hooks/useTranslate'

import { INVITE_HOURS, INVITE_USES, useAccessActions } from './graphql'
import { accessKey } from './keys'
import MadeLink, { type MadeLinkValue } from './MadeLink'

const errorMessage = (error: unknown) =>
	error instanceof Error && error.message ? error.message : undefined

/**
 * A link that gives one more phone or computer a pass for a person who already has an account:
 * what the old *Invite links* section did, now inside the person's own sheet. The address that
 * comes back is shown once, in the same block as a new person's invitation
 */
export default function DeviceLinkForm({
	personId,
	onClose,
}: {
	personId: string
	onClose: () => void
}) {
	const { t } = useTranslate()
	const actions = useAccessActions()
	const [label, setLabel] = useState('')
	const [hours, setHours] = useState<number>(72)
	const [uses, setUses] = useState<number>(3)
	const [missing, setMissing] = useState(false)
	const [made, setMade] = useState<MadeLinkValue | null>(null)

	const submit = async (event: FormEvent) => {
		event.preventDefault()
		if (!label.trim()) {
			setMissing(true)
			return
		}
		try {
			const url = await actions.createInvite({
				personId,
				label: label.trim(),
				validForHours: hours,
				maxUses: uses,
			})
			setMade({ url, label: label.trim() })
		} catch (error) {
			toast.error(t(accessKey('person.deviceLinkError')), { description: errorMessage(error) })
		}
	}

	if (made) {
		return (
			<MadeLink
				made={made}
				onDone={() => {
					// the address is forgotten with the block
					setMade(null)
					setLabel('')
					onClose()
				}}
			/>
		)
	}

	return (
		<form onSubmit={submit} data-testid="access-device-link-form" className="gap-4 flex flex-col">
			<Input
				id="access-device-link-label"
				label={t(accessKey('person.deviceName'))}
				placeholder={t(accessKey('person.deviceNamePlaceholder'))}
				value={label}
				onChange={(event) => {
					setLabel(event.target.value)
					setMissing(false)
				}}
				maxLength={40}
				fullWidth
				errorMessage={missing ? t(accessKey('person.deviceNameMissing')) : undefined}
			/>
			<div className="gap-4 sm:grid-cols-2 grid grid-cols-1">
				<div className="gap-1.5 flex flex-col">
					<label htmlFor="access-device-link-hours" className="text-sm font-medium">
						{t(accessKey('invite.worksFor'))}
					</label>
					<NativeSelect
						id="access-device-link-hours"
						value={hours}
						onChange={(event) => setHours(Number(event.target.value))}
						options={INVITE_HOURS.map((value) => ({
							label: t(accessKey(`invite.validity.h${value}`)),
							value,
						}))}
					/>
				</div>
				<div className="gap-1.5 flex flex-col">
					<label htmlFor="access-device-link-uses" className="text-sm font-medium">
						{t(accessKey('person.canBeOpened'))}
					</label>
					<NativeSelect
						id="access-device-link-uses"
						value={uses}
						onChange={(event) => setUses(Number(event.target.value))}
						options={INVITE_USES.map((value) => ({
							label: t(accessKey('person.times'), { count: value }),
							value,
						}))}
					/>
				</div>
			</div>
			<div className="gap-2 flex">
				<Button type="button" variant="outline" onClick={onClose}>
					{t(accessKey('invite.cancel'))}
				</Button>
				<Button type="submit" isLoading={actions.isPending}>
					{t(accessKey('invite.create'))}
				</Button>
			</div>
		</form>
	)
}
