import { NewCard, RawSwitch, Text } from '@stump/components'
import { extractErrorMessage } from '@stump/graphql'
import { Globe, Lock } from 'lucide-react'
import { useCallback } from 'react'
import { toast } from 'sonner'

import { useTranslate } from '@/hooks/useTranslate'

import { useReadingPeople, useUpdateReadingPrivacy } from './graphql'

/**
 * Whether the person lets everyone on the server see their reading, and the way to change it.
 * It is off until they turn it on, and turning it off takes effect for everyone at once
 */
function useReadingPrivacy() {
	const { t } = useTranslate()
	const { data, isLoading } = useReadingPeople()
	const { mutate, isPending } = useUpdateReadingPrivacy()

	const isSharing = data?.myReadingPrivacy.shareReadingHistory ?? false

	const setSharing = useCallback(
		(share: boolean) =>
			mutate(share, {
				onError: (error) =>
					toast.error(t('readingHistory.privacy.saveFailed'), {
						description: extractErrorMessage(error),
					}),
			}),
		[mutate, t],
	)

	return { isSharing, setSharing, isDisabled: isLoading || isPending }
}

/**
 * On the person's own page: who sees it now, and the switch. What it shares is told in the
 * words, so nobody turns it on without knowing
 */
export function ReadingPrivacyStrip() {
	const { t } = useTranslate()
	const { isSharing, setSharing, isDisabled } = useReadingPrivacy()
	const Icon = isSharing ? Globe : Lock

	return (
		<section
			aria-label={t('readingHistory.privacy.label')}
			className="gap-4 p-4 sm:flex-row sm:items-center sm:justify-between flex flex-col rounded-xl border border-border bg-card text-card-foreground"
		>
			<div className="gap-3 flex items-start">
				<Icon className="mt-0.5 size-5 shrink-0 text-muted-foreground" />
				<div className="gap-1 flex flex-col">
					<Text className="font-medium">
						{isSharing
							? t('readingHistory.privacy.sharedTitle')
							: t('readingHistory.privacy.privateTitle')}
					</Text>
					<Text variant="muted" size="sm" className="max-w-prose">
						{t('readingHistory.privacy.description')}
					</Text>
				</div>
			</div>

			<label className="gap-3 text-sm flex shrink-0 items-center">
				{t('readingHistory.privacy.switch')}
				<RawSwitch
					aria-label={t('readingHistory.privacy.switch')}
					checked={isSharing}
					disabled={isDisabled}
					onCheckedChange={setSharing}
				/>
			</label>
		</section>
	)
}

/** The same switch in Settings → Account, as a card among the others */
export function ReadingPrivacyCard() {
	const { t } = useTranslate()
	const { isSharing, setSharing, isDisabled } = useReadingPrivacy()

	return (
		<NewCard
			label={t('readingHistory.privacy.label')}
			description={t('readingHistory.privacy.cardDescription')}
		>
			<NewCard.Row
				label={t('readingHistory.privacy.switch')}
				description={t('readingHistory.privacy.description')}
				onClick={() => !isDisabled && setSharing(!isSharing)}
				className="flex-row items-center justify-between"
			>
				<RawSwitch
					aria-label={t('readingHistory.privacy.switch')}
					checked={isSharing}
					disabled={isDisabled}
					onCheckedChange={setSharing}
					// a click on the switch is the switch's own: the row would change it a second time
					onClick={(event) => event.stopPropagation()}
				/>
			</NewCard.Row>
		</NewCard>
	)
}
