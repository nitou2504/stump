import { useIntakeActions, useUploaderAgeLimit } from '@/scenes/intake/hooks'
import IntakeItemCard from '@/scenes/intake/IntakeItemCard'
import type { IntakeItemData } from '@/scenes/intake/queries'

type Props = {
	item: IntakeItemData
	libraries: Array<{ id: string; name: string }>
	actions: ReturnType<typeof useIntakeActions>
}

/**
 * A book somebody sent for approval, as the approver sees it: the whole card (where it goes, its
 * tags and the suggestions), with the warning that its age rating would hide it from the person
 * who sent it when their age limit is known
 */
export default function ReviewCard({ item, libraries, actions }: Props) {
	const uploaderAge = useUploaderAgeLimit(item.uploader?.id)
	const busy =
		actions.isReviewing || actions.isApproving || actions.isRejecting || actions.isChangingLibrary

	return (
		<IntakeItemCard
			mode="review"
			item={item}
			libraries={libraries}
			uploaderAge={uploaderAge}
			isBusy={busy}
			isSendingBack={actions.isRejecting}
			onApprove={actions.approveItem}
			onSendBack={actions.reject}
			onChangeLibrary={actions.changeLibrary}
			onReanalyse={actions.reanalyse}
			onReview={actions.reviewItem}
		/>
	)
}
