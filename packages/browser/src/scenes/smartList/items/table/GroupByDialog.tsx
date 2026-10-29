import { Button, cn, Dialog, Input, Text } from '@stump/components'
import { Media, MediaFilterInput, SmartListGrouping } from '@stump/graphql'
import { Layers } from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'

import { useFilterContext } from '@/components/filters/context'

import { useSmartListContext } from '../../context'
import {
	detectKeyedPrefixes,
	FIELD_LABELS,
	FieldGrouping,
	GROUP_FIELDS,
	groupBooksByField,
	groupingLabel,
	isSameGrouping,
} from '../../fieldGrouping'
import { useSmartListItems } from '../../graphql'

/** What the dialog has selected: a server grouping or a field grouping */
type Choice =
	| { kind: 'server'; grouping: SmartListGrouping }
	| { kind: 'field'; grouping: FieldGrouping }

const SERVER_OPTIONS = [
	{ label: 'None', value: SmartListGrouping.ByBooks },
	{ label: 'Series', value: SmartListGrouping.BySeries },
	{ label: 'Library', value: SmartListGrouping.ByLibrary },
]

const PREVIEW_GROUPS = 14

/** The current grouping's short label for the toolbar button */
export function useGroupingButtonLabel() {
	const { grouping, fieldGrouping } = useSmartListContext()
	if (fieldGrouping) return groupingLabel(fieldGrouping)
	return SERVER_OPTIONS.find(({ value }) => value === grouping)?.label ?? 'None'
}

type Props = {
	isOpen: boolean
	onClose: () => void
}

/**
 * Pick how a smart list's books are grouped, with a live preview of the groups. Besides
 * series and library, books can be grouped by any detail (author, language, rating, ...)
 * or by a `Key: *` pattern over their genres and tags; `Key: value` schemes found in the
 * list are suggested. These are the viewer's choices and don't change the list.
 */
export default function GroupByDialog({ isOpen, onClose }: Props) {
	const {
		list: { id },
		grouping,
		fieldGrouping,
		setGrouping,
		setFieldGrouping,
	} = useSmartListContext()

	const current = useMemo<Choice>(
		() =>
			fieldGrouping ? { grouping: fieldGrouping, kind: 'field' } : { grouping, kind: 'server' },
		[fieldGrouping, grouping],
	)
	const [choice, setChoice] = useState<Choice>(current)
	const [pattern, setPattern] = useState('')

	useEffect(() => {
		if (isOpen) {
			setChoice(current)
			setPattern(
				current.kind === 'field' && current.grouping.field === 'pattern'
					? current.grouping.pattern
					: '',
			)
		}
	}, [isOpen, current])

	// The same books as the list shows (with its ad-hoc filters), ungrouped, for the preview
	const { filters } = useFilterContext()
	const { items, isLoading } = useSmartListItems({
		enabled: isOpen,
		filter: filters as MediaFilterInput,
		grouping: SmartListGrouping.ByBooks,
		id,
	})
	const books = useMemo(() => (items && 'books' in items ? (items.books as Media[]) : []), [items])
	const prefixes = useMemo(() => detectKeyedPrefixes(books), [books])

	const preview = useMemo(
		() => (choice.kind === 'field' ? groupBooksByField(books, choice.grouping) : null),
		[books, choice],
	)

	const apply = (next: Choice = choice) => {
		if (next.kind === 'field') {
			setFieldGrouping(next.grouping)
		} else {
			setGrouping(next.grouping)
		}
		onClose()
	}

	const isFieldSelected = (grouping: FieldGrouping) =>
		choice.kind === 'field' && isSameGrouping(choice.grouping, grouping)

	const selectPattern = (value: string) => {
		setPattern(value)
		if (value.trim()) {
			setChoice({ grouping: { field: 'pattern', pattern: value }, kind: 'field' })
		}
	}

	const maxCount = Math.max(1, ...(preview?.groups.map((group) => group.books.length) ?? []))

	return (
		<Dialog open={isOpen} onOpenChange={(open) => !open && onClose()}>
			<Dialog.Content size="xl">
				<Dialog.Header>
					<Dialog.Title>Group books by</Dialog.Title>
					<Dialog.Description>
						Only changes how you see this list. Double-click an option to apply it at once.
					</Dialog.Description>
					<Dialog.Close onClick={onClose} />
				</Dialog.Header>

				<div className="gap-4 md:grid-cols-[1fr_1fr] grid grid-cols-1">
					<div className="gap-4 flex flex-col">
						<OptionSection title="Collections">
							{SERVER_OPTIONS.map(({ label, value }) => {
								const option: Choice = { grouping: value, kind: 'server' }
								return (
									<OptionChip
										key={value}
										label={label}
										isSelected={choice.kind === 'server' && choice.grouping === value}
										onSelect={() => setChoice(option)}
										onApply={() => apply(option)}
									/>
								)
							})}
						</OptionSection>

						<OptionSection title="Book details">
							{GROUP_FIELDS.map((field) => {
								const option: Choice = { grouping: { field }, kind: 'field' }
								return (
									<OptionChip
										key={field}
										label={FIELD_LABELS[field]}
										isSelected={isFieldSelected({ field })}
										onSelect={() => setChoice(option)}
										onApply={() => apply(option)}
									/>
								)
							})}
						</OptionSection>

						{prefixes.length > 0 && (
							<OptionSection title="Found in this list">
								{prefixes.map(({ prefix, pattern: prefixPattern, values, books: count }) => {
									const option: Choice = {
										grouping: { field: 'pattern', pattern: prefixPattern },
										kind: 'field',
									}
									return (
										<OptionChip
											key={prefix}
											label={`${prefix}: *`}
											hint={`${values} values · ${count} books`}
											isSelected={isFieldSelected(option.grouping)}
											onSelect={() => {
												setPattern(prefixPattern)
												setChoice(option)
											}}
											onApply={() => apply(option)}
										/>
									)
								})}
							</OptionSection>
						)}

						<div>
							<Text size="sm" className="mb-1.5 font-medium">
								Custom pattern
							</Text>
							<Input
								placeholder="e.g. Asignatura: *"
								value={pattern}
								onChange={(e) => selectPattern(e.target.value)}
								onKeyDown={(e) => {
									if (e.key === 'Enter' && pattern.trim()) {
										apply({ grouping: { field: 'pattern', pattern }, kind: 'field' })
									}
								}}
							/>
							<Text size="xs" variant="muted" className="mt-1">
								Matched against genres and tags; <code>*</code> becomes the group name.
							</Text>
						</div>
					</div>

					<div className="p-3 flex min-h-[260px] flex-col rounded-lg border border-border bg-muted/40">
						<Text size="sm" className="mb-2 font-medium">
							Preview
						</Text>
						{choice.kind === 'server' ? (
							<Text size="sm" variant="muted">
								{choice.grouping === SmartListGrouping.ByBooks
									? 'All books in one list, without groups.'
									: `One group per ${choice.grouping === SmartListGrouping.BySeries ? 'series' : 'library'}, as stored in Stump.`}
							</Text>
						) : isLoading && !books.length ? (
							<Text size="sm" variant="muted">
								Loading books…
							</Text>
						) : preview ? (
							<div className="gap-1.5 flex flex-col">
								<Text size="xs" variant="muted">
									{preview.groups.length} groups · {books.length - preview.ungrouped.length} of{' '}
									{books.length} books
									{preview.hasMultiMembership ? ' · some books are in several groups' : ''}
								</Text>
								{preview.groups.length === 0 && (
									<Text size="sm" variant="muted" className="py-4">
										No book matches. Try another field or pattern.
									</Text>
								)}
								{preview.groups.slice(0, PREVIEW_GROUPS).map((group) => (
									<PreviewRow
										key={group.key}
										name={group.name}
										count={group.books.length}
										max={maxCount}
									/>
								))}
								{preview.groups.length > PREVIEW_GROUPS && (
									<Text size="xs" variant="muted">
										+ {preview.groups.length - PREVIEW_GROUPS} more groups
									</Text>
								)}
								{preview.ungrouped.length > 0 && (
									<PreviewRow
										name={`No ${groupingLabel(choice.grouping)}`}
										count={preview.ungrouped.length}
										max={maxCount}
										isMuted
									/>
								)}
							</div>
						) : null}
					</div>
				</div>

				<Dialog.Footer>
					<Button variant="ghost" onClick={onClose}>
						Cancel
					</Button>
					<Button onClick={() => apply()}>Apply</Button>
				</Dialog.Footer>
			</Dialog.Content>
		</Dialog>
	)
}

/** Toolbar button showing the current grouping; opens the dialog */
export function GroupByButton({ onClick }: { onClick: () => void }) {
	const label = useGroupingButtonLabel()
	return (
		<Button
			variant="outline"
			size="sm"
			className="h-8 gap-1.5 max-w-[200px] shrink-0"
			onClick={onClick}
			title="Group books by…"
		>
			<Layers className="h-4 w-4 shrink-0" />
			<span className="truncate">Group: {label}</span>
		</Button>
	)
}

function OptionSection({ title, children }: { title: string; children: React.ReactNode }) {
	return (
		<div>
			<Text size="sm" className="mb-1.5 font-medium">
				{title}
			</Text>
			<div className="gap-1.5 flex flex-wrap">{children}</div>
		</div>
	)
}

type OptionChipProps = {
	label: string
	hint?: string
	isSelected: boolean
	onSelect: () => void
	onApply: () => void
}

function OptionChip({ label, hint, isSelected, onSelect, onApply }: OptionChipProps) {
	return (
		<button
			type="button"
			aria-pressed={isSelected}
			onClick={onSelect}
			onDoubleClick={onApply}
			title={hint}
			className={cn(
				'px-2.5 py-1 text-sm rounded-full border transition-colors',
				isSelected
					? 'border-primary bg-primary text-primary-foreground'
					: 'border-border bg-background hover:bg-muted',
			)}
		>
			{label}
			{hint && (
				<span className={cn('ml-1.5 text-xs', isSelected ? 'opacity-80' : 'text-muted-foreground')}>
					{hint}
				</span>
			)}
		</button>
	)
}

function PreviewRow({
	name,
	count,
	max,
	isMuted,
}: {
	name: string
	count: number
	max: number
	isMuted?: boolean
}) {
	return (
		<div className="gap-2 text-sm flex items-center">
			<span
				className={cn('w-2/5 truncate', isMuted && 'text-muted-foreground italic')}
				title={name}
			>
				{name}
			</span>
			<div className="h-2 flex-1 overflow-hidden rounded-full bg-background">
				<div
					className={cn(
						'h-full rounded-full',
						isMuted ? 'bg-muted-foreground/40' : 'bg-primary/70',
					)}
					style={{ width: `${Math.max(4, (count / max) * 100)}%` }}
				/>
			</div>
			<span className="w-10 text-right text-muted-foreground tabular-nums">{count}</span>
		</div>
	)
}
