import { NewCard, RawSwitch } from '@stump/components'
import { UserPermission } from '@stump/graphql'
import { useCallback } from 'react'

import { useAppContext } from '@/context'
import { usePreferences } from '@/hooks/usePreferences'

/**
 * Per-user opt-in for the "Remove from library" action on book pages. Only offered to
 * users who can manage libraries, since the server requires that permission anyway.
 */
export default function EnableBookRemoval() {
	const { checkPermission } = useAppContext()
	const {
		preferences: { enableBookRemoval },
		update,
	} = usePreferences()

	const handleChange = useCallback(() => {
		update({ enableBookRemoval: !enableBookRemoval })
	}, [enableBookRemoval, update])

	if (!checkPermission(UserPermission.ManageLibrary)) return null

	return (
		<NewCard.Row
			label="Allow removing books"
			description="Show “Remove from library” in a book's menu. The file stays on disk and the book can be restored from the library's settings (Danger zone → Deletion)."
			onClick={handleChange}
			className="flex-row items-center justify-between"
		>
			<RawSwitch
				id="enableBookRemoval"
				checked={!!enableBookRemoval}
				onCheckedChange={handleChange}
			/>
		</NewCard.Row>
	)
}
