import { cn, Input } from '@stump/components'
import { IntakeSource } from '@stump/graphql'
import { useEffect, useState } from 'react'

import { sourceLabel } from './reviewState'

/** A tiny tag saying where a value came from (File, Open Library, You, ...) */
export function SourceBadge({ source }: { source?: IntakeSource }) {
	const label = sourceLabel(source)
	if (!label) return null
	return (
		<span
			className={cn(
				'px-1.5 rounded leading-4 font-medium py-px text-[10px] whitespace-nowrap',
				source === IntakeSource.User
					? 'bg-primary/15 text-primary'
					: source === IntakeSource.OpenLibrary || source === IntakeSource.GoogleBooks
						? 'bg-sky-500/15 text-sky-600 dark:text-sky-400'
						: 'bg-muted text-muted-foreground',
			)}
		>
			{label}
		</span>
	)
}

type FieldProps = {
	label: string
	value: string
	/** Called when the user leaves the field (or presses Enter) with a changed value */
	onCommit: (value: string) => void
	source?: IntakeSource
	placeholder?: string
	disabled?: boolean
	className?: string
	inputMode?: 'text' | 'numeric' | 'decimal'
	hint?: string
	invalid?: boolean
}

/** A compact labelled input that saves on blur / Enter, with the value's source */
export function ReviewField({
	label,
	value,
	onCommit,
	source,
	placeholder,
	disabled,
	className,
	inputMode,
	hint,
	invalid,
}: FieldProps) {
	const [draft, setDraft] = useState(value)

	// follow the server's value (e.g. after a re-analysis) unless the user is typing
	useEffect(() => {
		setDraft(value)
	}, [value])

	const commit = () => {
		if (draft.trim() !== value.trim()) onCommit(draft)
	}

	return (
		<label className={cn('gap-1 min-w-0 flex flex-col', className)}>
			<span className="gap-1.5 text-xs flex items-center text-muted-foreground">
				{label}
				<SourceBadge source={source} />
			</span>
			<Input
				value={draft}
				placeholder={placeholder}
				disabled={disabled}
				inputMode={inputMode}
				title={hint}
				size="sm"
				fullWidth
				isInvalid={invalid}
				onChange={(e) => setDraft(e.target.value)}
				onBlur={commit}
				onKeyDown={(e) => {
					if (e.key === 'Enter') {
						e.preventDefault()
						;(e.target as HTMLInputElement).blur()
					} else if (e.key === 'Escape') {
						setDraft(value)
					}
				}}
			/>
		</label>
	)
}
