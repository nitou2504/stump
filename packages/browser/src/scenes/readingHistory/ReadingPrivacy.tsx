import { Button, cn, NewCard, Popover, RawSwitch, Text } from '@stump/components'
import { extractErrorMessage } from '@stump/graphql'
import { Globe, Lock } from 'lucide-react'
import { useCallback, useId } from 'react'
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
	// until it is known, "private" would be a guess, and a wrong one for someone who shares
	const isReady = !!data

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

	return { isReady, isSharing, setSharing, isDisabled: isLoading || isPending }
}

/**
 * On the person's own page, at the end of the header: a small chip that says who sees the page
 * now (a lock for only them, a globe for everyone on the server). It opens the switch with what
 * is shared told in the words under it, so nobody turns it on without knowing, and the page
 * below it is all reading. On a narrow phone only the icon is left, and its name says the rest
 */
export function ReadingPrivacyControl() {
	const { t } = useTranslate()
	const { isReady, isSharing, setSharing, isDisabled } = useReadingPrivacy()
	const switchId = useId()
	// the switch is where focus lands when the panel opens: it reads what is shared before it is turned on
	const descriptionId = useId()

	if (!isReady) return null

	const Icon = isSharing ? Globe : Lock

	return (
		<Popover>
			<Popover.Trigger asChild>
				<Button
					variant="outline"
					size="sm"
					rounded="full"
					aria-label={
						isSharing
							? t('readingHistory.privacy.sharedLabel')
							: t('readingHistory.privacy.privateLabel')
					}
					className={cn(
						'gap-1.5 px-3 text-muted-foreground',
						// who can see it is worth a second look when it is everyone
						isSharing && 'text-foreground',
					)}
				>
					<Icon aria-hidden />
					<span aria-hidden className="hidden min-[360px]:inline">
						{isSharing ? t('readingHistory.privacy.shared') : t('readingHistory.privacy.private')}
					</span>
				</Button>
			</Popover.Trigger>

			<Popover.Content
				portal
				align="end"
				collisionPadding={12}
				aria-label={t('readingHistory.privacy.label')}
				className="w-72 max-w-[calc(100vw-1.5rem)]"
			>
				<div className="gap-3 flex flex-col">
					<div className="gap-4 flex items-center justify-between">
						<label htmlFor={switchId} className="text-sm font-medium leading-snug">
							{t('readingHistory.privacy.switch')}
						</label>
						<RawSwitch
							id={switchId}
							aria-describedby={descriptionId}
							checked={isSharing}
							disabled={isDisabled}
							onCheckedChange={setSharing}
						/>
					</div>
					<Text id={descriptionId} variant="muted" size="sm">
						{t('readingHistory.privacy.description')}
					</Text>
				</div>
			</Popover.Content>
		</Popover>
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
