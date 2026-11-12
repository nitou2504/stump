import { Heading, Text } from '@stump/components'
import type { PropsWithChildren, ReactNode } from 'react'

type Props = PropsWithChildren<{
	/** Names the section for tests and for the heading that labels it */
	id: string
	title: string
	description?: string
	/** What goes at the far end of the heading (a toggle, a button) */
	actions?: ReactNode
}>

/**
 * One block of the page: a heading with what it is for, then its content. The same shape as the
 * other settings pages' sections (the Users page's login history, for one)
 */
export default function Section({ id, title, description, actions, children }: Props) {
	return (
		<section
			id={`access-${id}`}
			data-testid={`access-${id}`}
			aria-labelledby={`access-${id}-title`}
			className="gap-y-4 flex flex-col"
		>
			<div className="gap-3 flex flex-wrap items-end justify-between">
				<div className="min-w-0">
					<Heading as="h2" size="sm" id={`access-${id}-title`}>
						{title}
					</Heading>
					{description && (
						<Text size="sm" variant="muted" className="mt-1">
							{description}
						</Text>
					)}
				</div>
				{actions}
			</div>
			{children}
		</section>
	)
}

/** A bordered block whose rows are separated by lines: the page's lists */
export function RowList({ children, ...props }: PropsWithChildren<{ 'data-testid'?: string }>) {
	return (
		<div className="divide-y divide-border rounded-lg bg-card ring-1 ring-foreground/10" {...props}>
			{children}
		</div>
	)
}

/** A row of a [`RowList`]: its parts sit side by side, and stack on a phone */
export function Row({ children, ...props }: PropsWithChildren<{ 'data-testid'?: string }>) {
	return (
		<div
			className="gap-x-6 gap-y-2 px-4 py-3 md:flex-row md:items-center flex flex-col justify-between"
			{...props}
		>
			{children}
		</div>
	)
}

/** What a list says when it has nothing in it */
export function Empty({ children }: PropsWithChildren) {
	return (
		<Text size="sm" variant="muted" className="px-4 py-6 text-center">
			{children}
		</Text>
	)
}
