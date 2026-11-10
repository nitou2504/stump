import { IntakeState } from '@stump/graphql'
import { render, screen } from '@testing-library/react'

import IntakeStatusChip, { intakeStatus, type IntakeStatusKind } from '../IntakeStatus'
import type { IntakeItemData } from '../queries'
import { item } from './fixtures'

const CASES: Array<[Partial<IntakeItemData>, IntakeStatusKind, string]> = [
	[{ state: IntakeState.Analysing }, 'reading', 'Reading…'],
	[{ state: IntakeState.Failed }, 'failed', 'Couldn’t read it'],
	[{ state: IntakeState.Filed }, 'added', 'Added'],
	[{ state: IntakeState.Filing }, 'adding', 'Adding…'],
	[{ state: IntakeState.PendingApproval }, 'waiting', 'Waiting for approval'],
	[{ reviewNote: 'Fix it', state: IntakeState.Rejected }, 'sentBack', 'Sent back'],
	[{ state: IntakeState.NeedsReview }, 'check', 'Needs your details'],
	[{ problems: ['Add an author'], state: IntakeState.Ready }, 'check', 'Needs your details'],
	[{ state: IntakeState.Ready }, 'ready', 'Ready'],
]

describe('intakeStatus', () => {
	it.each(CASES)('says %j is %s: “%s”', (overrides, kind, label) => {
		expect(intakeStatus(item(overrides))).toMatchObject({ kind, label })
	})

	it('keeps a sent back book “sent back” after the uploader changed it', () => {
		expect(
			intakeStatus(item({ reviewNote: 'Add the series', state: IntakeState.Ready })).kind,
		).toBe('sentBack')
	})

	it('says ready books are ready to send for somebody who needs approval', () => {
		expect(intakeStatus(item(), { friend: true }).label).toBe('Ready to send')
	})

	it('says everything an approver sees is to approve', () => {
		for (const state of [IntakeState.PendingApproval, IntakeState.Ready]) {
			expect(intakeStatus(item({ state }), { reviewing: true })).toMatchObject({
				kind: 'toApprove',
				label: 'To approve',
			})
		}
	})
})

describe('IntakeStatusChip', () => {
	it('says it in a word, with an icon, so colour is never the only signal', () => {
		const { container } = render(
			<IntakeStatusChip item={item({ state: IntakeState.NeedsReview })} />,
		)

		expect(screen.getByText('Needs your details')).toBeInTheDocument()
		expect(container.querySelector('svg')).toHaveAttribute('aria-hidden', 'true')
	})

	it('spins while a book is being read or added', () => {
		const { container, rerender } = render(
			<IntakeStatusChip item={item({ state: IntakeState.Analysing })} />,
		)
		expect(container.querySelector('svg')).toHaveClass('animate-spin')

		rerender(<IntakeStatusChip item={item({ state: IntakeState.Filing })} />)
		expect(container.querySelector('svg')).toHaveClass('animate-spin')

		rerender(<IntakeStatusChip item={item({ state: IntakeState.Ready })} />)
		expect(container.querySelector('svg')).not.toHaveClass('animate-spin')
	})
})
