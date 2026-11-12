import { Alert, AlertDescription, AlertTitle, Button, Link } from '@stump/components'
import { IntakeState } from '@stump/graphql'
import { formatDistanceToNow } from 'date-fns'
import { AlertTriangle, CircleAlert, Copy, MessageSquareWarning } from 'lucide-react'
import { useState } from 'react'

import { useTranslate } from '@/hooks/useTranslate'
import { usePaths } from '@/paths'

import type { IntakeItemData } from './queries'
import { type AgeLimit, ageRatingHides } from './reviewState'

/** The warnings the server writes when a place to look a book up could not be reached */
const SOURCE_FAILURE = /: (unreachable|no answer in time)/i

/** The server's warning that goes with a duplicate, which the duplicates alert already says */
const DUPLICATE_WARNING = 'This book may already be in the library'

type Looking = {
	friend: boolean
	reviewing: boolean
}

type Props = Looking & {
	item: IntakeItemData
	/** Whether an approver sent the book back (the note stays until it is sent again) */
	sentBack: boolean
	uploaderAge?: AgeLimit | null
	isBusy?: boolean
	onChangeLibrary: (id: string, libraryId: string) => void
}

/**
 * What a card has to say before anything else: an approver's note, a failure, a rating that
 * would hide the book from whoever sent it, a possible duplicate, the server's warnings and a
 * library of its own for the book. Each is shown only when it is true, so most cards say nothing.
 * (A failure's way out, *Try again*, is the card's main button, not a button in here.)
 */
export function CardNotices({
	item,
	friend,
	reviewing,
	sentBack,
	uploaderAge,
	isBusy,
	onChangeLibrary,
}: Props) {
	const paths = usePaths()
	const { t } = useTranslate()
	const { proposal, decision } = item
	const suggested = item.suggestedLibrary
	const ageHides = reviewing ? ageRatingHides(decision?.metadata.ageRating, uploaderAge) : null
	const uploaderName = item.uploader?.username ?? 'The person who sent it'
	// the duplicates have their own alert
	const allWarnings = (proposal?.warnings ?? []).filter(
		(warning) => !proposal?.duplicates.length || warning !== DUPLICATE_WARNING,
	)
	// a source that could not be asked ("Open Library: unreachable") is not something to fix: one
	// muted line for an approver, and nothing at all for the person who sent the book
	const sourceFailures = allWarnings.filter((warning) => SOURCE_FAILURE.test(warning))
	const warnings = allWarnings.filter((warning) => !SOURCE_FAILURE.test(warning))
	const [showSources, setShowSources] = useState(false)

	return (
		<>
			{sentBack && !reviewing && (
				<Alert variant="warning">
					<MessageSquareWarning />
					<AlertTitle>
						Sent back
						{item.reviewedAt &&
							` ${formatDistanceToNow(new Date(item.reviewedAt), { addSuffix: true })}`}
					</AlertTitle>
					<AlertDescription>
						{item.reviewNote || 'Change what is needed and send it again.'}
					</AlertDescription>
				</Alert>
			)}

			{item.state === IntakeState.Failed && (
				<Alert variant="destructive">
					<CircleAlert />
					<AlertTitle>What went wrong</AlertTitle>
					<AlertDescription>{item.error || 'Unknown error'}</AlertDescription>
				</Alert>
			)}

			{/* a rating that would keep the book from the person who sent it */}
			{reviewing && ageHides && (
				<Alert variant="warning">
					<AlertTriangle />
					<AlertTitle>
						{ageHides === 'hidden'
							? `${uploaderName} couldn’t read this book`
							: `${uploaderName} may not be able to read this book`}
					</AlertTitle>
					<AlertDescription>
						{ageHides === 'hidden'
							? `Their age limit is ${uploaderAge?.age}, and the book is rated ${decision?.metadata.ageRating}+. They would not see it in the library.`
							: 'They can only read books with an age rating, and this one has none.'}
					</AlertDescription>
				</Alert>
			)}

			{!!proposal?.duplicates.length && (
				<Alert variant="warning">
					<Copy />
					<AlertTitle>Already in the library?</AlertTitle>
					<AlertDescription>
						<ul className="gap-1 flex flex-col">
							{proposal.duplicates.map((dup) => (
								<li
									key={`${dup.mediaId}:${dup.fileName}`}
									title={friend ? undefined : dup.path || undefined}
								>
									{friend ? (
										<span className="font-medium">{dup.title}</span>
									) : (
										<Link to={paths.bookOverview(dup.mediaId)}>{dup.title}</Link>
									)}{' '}
									<span className="text-muted-foreground">
										({dup.reason === 'SAME_FILE' ? 'same file' : 'same author and title'})
									</span>
								</li>
							))}
						</ul>
					</AlertDescription>
				</Alert>
			)}

			{warnings.length > 0 && (
				<ul className="gap-1 text-xs flex flex-col text-muted-foreground">
					{warnings.map((warning) => (
						<li key={warning} className="gap-1.5 flex items-start">
							<AlertTriangle className="h-3.5 w-3.5 mt-px shrink-0 text-warning" aria-hidden />
							{warning}
						</li>
					))}
				</ul>
			)}

			{!friend && sourceFailures.length > 0 && (
				<div
					className="gap-1 text-xs flex flex-col text-muted-foreground"
					data-testid="intake-source-failures"
				>
					<span className="gap-2 flex flex-wrap items-center">
						{t('intakeScene.sourceFailures', { count: sourceFailures.length })}
						<button
							type="button"
							className="underline"
							aria-expanded={showSources}
							onClick={() => setShowSources((open) => !open)}
						>
							{t('intakeScene.sourceDetails')}
						</button>
					</span>
					{showSources && (
						<ul className="pl-4 list-disc">
							{sourceFailures.map((warning) => (
								<li key={warning}>{warning}</li>
							))}
						</ul>
					)}
				</div>
			)}

			{suggested && (
				<div className="gap-2 text-xs flex flex-wrap items-center">
					<Button
						size="xs"
						variant="secondary"
						disabled={isBusy}
						onClick={() => onChangeLibrary(item.id, suggested.id)}
					>
						Move to {suggested.name}
					</Button>
					<span className="text-muted-foreground">It is read again there</span>
				</div>
			)}
		</>
	)
}

/**
 * What keeps the book from going on, right above the button that can't be pressed: the server's
 * list of what is left to fix, and why the last try at adding it failed
 */
export function BlockingNotices({
	item,
	friend,
	reviewing,
}: Looking & { item: Pick<IntakeItemData, 'problems' | 'error' | 'state'> }) {
	return (
		<>
			{item.problems.length > 0 && (
				<Alert variant="destructive">
					<CircleAlert />
					<AlertTitle>
						{friend
							? 'Before it can be sent'
							: reviewing
								? 'Before it can be approved'
								: 'Before it can be added'}
					</AlertTitle>
					<AlertDescription>
						<ul className="pl-4 list-disc">
							{item.problems.map((problem) => (
								<li key={problem}>{problem}</li>
							))}
						</ul>
					</AlertDescription>
				</Alert>
			)}

			{item.error && item.state !== IntakeState.Failed && (
				<Alert variant="destructive">
					<CircleAlert />
					<AlertTitle>Adding it failed</AlertTitle>
					<AlertDescription>{item.error}</AlertDescription>
				</Alert>
			)}
		</>
	)
}
