import { Dialog, NewCard, Text } from '@stump/components'

import { useTranslate } from '@/hooks/useTranslate'

import { formatWhen } from './format'
import { useAccessOverview } from './graphql'
import { accessKey } from './keys'

type Props = { isOpen: boolean; onClose: () => void }

/** The technical side of the page: what is set up, in numbers, for the day something is wrong */
export default function DetailsDialog({ isOpen, onClose }: Props) {
	const { t } = useTranslate()
	const overview = useAccessOverview()

	return (
		<Dialog open={isOpen} onOpenChange={(open) => !open && onClose()}>
			<Dialog.Content size="md" data-testid="access-details">
				<Dialog.Header>
					<Dialog.Title>{t(accessKey('details.title'))}</Dialog.Title>
					<Dialog.Close onClick={onClose} />
				</Dialog.Header>
				<NewCard>
					<NewCard.Row
						label={t(accessKey('details.address'))}
						value={overview.publicUrl ?? t(accessKey('details.none'))}
					/>
					<NewCard.Row
						label={t(accessKey('details.pass'))}
						value={t(accessKey(overview.requirePass ? 'details.yes' : 'details.no'))}
					/>
					<NewCard.Row
						label={t(accessKey('details.signin'))}
						value={t(accessKey(overview.publicSignin ? 'details.signinOn' : 'details.signinOff'))}
					/>
					<NewCard.Row
						label={t(accessKey('details.edge'))}
						value={
							overview.stateWrittenAt
								? formatWhen(overview.stateWrittenAt)
								: t(accessKey('details.never'))
						}
					/>
					<NewCard.Row
						label={t(accessKey('details.failures'))}
						value={overview.signinStats.failuresLast10Min}
					/>
					<NewCard.Row
						label={t(accessKey('details.slowed'))}
						value={overview.signinStats.namesWaiting}
					/>
				</NewCard>
				<Text size="sm" variant="muted">
					{t(accessKey('details.edgeNote'))}
				</Text>
			</Dialog.Content>
		</Dialog>
	)
}
