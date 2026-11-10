import { Avatar } from '@stump/components'
import { formatDistanceToNow } from 'date-fns'

type Props = {
	uploader?: { username: string; avatarUrl?: string | null } | null
	/** When the book was sent for approval */
	submittedAt?: string | null
}

/** Who sent a book for approval, and how long ago: the first thing an approver wants to know */
export default function UploaderRow({ uploader, submittedAt }: Props) {
	const name = uploader?.username ?? 'Someone'
	return (
		<span className="gap-1.5 text-xs flex items-center text-muted-foreground">
			<Avatar
				className="h-5 w-5"
				src={uploader?.avatarUrl ?? undefined}
				fallback={name}
				aria-hidden
			/>
			<span>
				from <span className="font-medium text-foreground">{name}</span>
				{submittedAt && (
					<> · sent {formatDistanceToNow(new Date(submittedAt), { addSuffix: true })}</>
				)}
			</span>
		</span>
	)
}
