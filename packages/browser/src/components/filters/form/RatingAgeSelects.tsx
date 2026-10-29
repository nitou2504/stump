import { Label, NativeSelect } from '@stump/components'
import { useFormContext } from 'react-hook-form'

import { MediaFilterFormSchema } from './MediaFilterForm'

/** The minimum ratings offered by the filter, on the 0-5 scale */
export const RATING_STEPS = ['4.5', '4', '3.5', '3'] as const

/**
 * Minimum rating and age (general / adult) selects for the book filter form
 */
export default function RatingAgeSelects() {
	const form = useFormContext<MediaFilterFormSchema>()

	return (
		<div className="gap-3 py-1.5 grid grid-cols-2">
			<div>
				<Label htmlFor="minRating" className="mb-1.5">
					Rating
				</Label>
				<NativeSelect
					id="minRating"
					options={[
						{ label: 'Any rating', value: '' },
						...RATING_STEPS.map((step) => ({ label: `★ ${step} or more`, value: step })),
					]}
					{...form.register('minRating')}
				/>
			</div>
			<div>
				<Label htmlFor="age" className="mb-1.5">
					Age
				</Label>
				<NativeSelect
					id="age"
					options={[
						{ label: 'Any age', value: '' },
						{ label: 'General (no 18+)', value: 'general' },
						{ label: 'Adult (18+)', value: 'adult' },
					]}
					{...form.register('age')}
				/>
			</div>
		</div>
	)
}
