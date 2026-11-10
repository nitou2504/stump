import { Badge } from '@stump/components'
import { IntakeState } from '@stump/graphql'
import {
	CheckCircle2,
	CircleAlert,
	ClipboardCheck,
	Hourglass,
	Loader2,
	MessageSquareWarning,
	Send,
} from 'lucide-react'

import type { IntakeItemData } from './queries'
import { isSentBack, needsAttention } from './reviewState'

export type IntakeStatusKind =
	| 'reading'
	| 'check'
	| 'ready'
	| 'failed'
	| 'sentBack'
	| 'waiting'
	| 'adding'
	| 'added'
	| 'toApprove'

type Variant = 'success' | 'warning' | 'error' | 'secondary' | 'primary'

type Looking = {
	/** Somebody who needs approval: their books are sent, not added */
	friend?: boolean
	/** An approver looking at a book somebody sent */
	reviewing?: boolean
}

/**
 * Where a book stands, in a few words, for whoever is looking: one of a handful of statuses
 * (reading, needs your details, ready, sent back, waiting for approval, being added, added),
 * so a card, a row and the page's sections all say the same thing
 */
export function intakeStatus(
	item: Pick<IntakeItemData, 'state' | 'problems' | 'proposal' | 'reviewNote'>,
	{ friend, reviewing }: Looking = {},
): { kind: IntakeStatusKind; label: string; variant: Variant } {
	if (reviewing) return { kind: 'toApprove', label: 'To approve', variant: 'primary' }
	switch (item.state) {
		case IntakeState.Analysing:
			return { kind: 'reading', label: 'Reading…', variant: 'secondary' }
		case IntakeState.Failed:
			return { kind: 'failed', label: 'Couldn’t read it', variant: 'error' }
		case IntakeState.Filed:
			return { kind: 'added', label: 'Added', variant: 'success' }
		case IntakeState.Filing:
			return { kind: 'adding', label: 'Adding…', variant: 'success' }
		case IntakeState.PendingApproval:
			return { kind: 'waiting', label: 'Waiting for approval', variant: 'secondary' }
		default:
			break
	}
	if (isSentBack(item)) return { kind: 'sentBack', label: 'Sent back', variant: 'warning' }
	if (needsAttention(item))
		return { kind: 'check', label: 'Needs your details', variant: 'warning' }
	return { kind: 'ready', label: friend ? 'Ready to send' : 'Ready', variant: 'success' }
}

const ICONS: Record<IntakeStatusKind, React.ComponentType<{ className?: string }>> = {
	added: CheckCircle2,
	adding: Loader2,
	check: ClipboardCheck,
	failed: CircleAlert,
	ready: Send,
	reading: Loader2,
	sentBack: MessageSquareWarning,
	toApprove: Hourglass,
	waiting: Hourglass,
}

type Props = Looking & {
	item: Pick<IntakeItemData, 'state' | 'problems' | 'proposal' | 'reviewNote'>
	className?: string
}

/** The status of a book as a chip: an icon and a word, so the colour is never the only signal */
export default function IntakeStatusChip({ item, friend, reviewing, className }: Props) {
	const { kind, label, variant } = intakeStatus(item, { friend, reviewing })
	const Icon = ICONS[kind]
	const spinning = kind === 'reading' || kind === 'adding'
	return (
		<Badge variant={variant} size="md" className={className}>
			<Icon className={spinning ? 'h-3 w-3 animate-spin' : 'h-3 w-3'} aria-hidden />
			{label}
		</Badge>
	)
}
