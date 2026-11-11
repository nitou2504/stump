import { NativeSelect } from '@stump/components'

/** The ratings of the Edit details dialog */
const AGE_OPTIONS = [
	{ label: 'All ages', value: '0' },
	{ label: '13+', value: '13' },
	{ label: '16+', value: '16' },
	{ label: '18+', value: '18' },
]

type Props = {
	value?: number | null
	onChange: (ageRating: number) => void
	disabled?: boolean
	className?: string
}

/**
 * A book's age rating. A rating can be set, not taken away (the server keeps the one that is
 * there when none is sent), so "Not set" is only shown for a book that has none
 */
export default function AgeRatingField({ value, onChange, disabled, className }: Props) {
	const current = value === null || value === undefined ? '' : String(value)
	const options = [
		...(current === '' ? [{ label: 'Not set', value: '' }] : []),
		...AGE_OPTIONS,
		// a rating of the file that is none of the usual ones
		...(current !== '' && !AGE_OPTIONS.some((o) => o.value === current)
			? [{ label: `${current}+`, value: current }]
			: []),
	]
	return (
		<label className={className}>
			<span className="text-xs text-muted-foreground">Age rating</span>
			<NativeSelect
				size="sm"
				className="text-base md:text-sm"
				value={current}
				disabled={disabled}
				options={options}
				onChange={(e) => {
					if (e.target.value !== '' && e.target.value !== current) onChange(Number(e.target.value))
				}}
			/>
		</label>
	)
}
