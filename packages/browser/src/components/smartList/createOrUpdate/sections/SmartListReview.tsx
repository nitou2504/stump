import { NewCard } from '@stump/components'
import { useFormContext, useWatch } from 'react-hook-form'

import { ReviewItem, ReviewSection } from '@/components/steppedForm'
import { useTranslate } from '@/hooks/useTranslate'

import { SmartListFormSchema } from '../schema'
import FilterConfigJSON from './FilterConfigJSON'

export default function SmartListReview() {
	const form = useFormContext<SmartListFormSchema>()
	const state = useWatch({ control: form.control })

	const { t } = useTranslate()

	return (
		<>
			<ReviewSection stepIndex={0}>
				<ReviewItem label={t(getLabelKey('name'))} value={state.name} />

				<ReviewItem label={t(getLabelKey('description'))} value={state.description} />

				<ReviewItem
					label={t(getLabelKey('visibility'))}
					value={
						state.visibility &&
						t(`${FIELDS_KEY}.visibility.options.${state.visibility.toLowerCase()}.label`)
					}
				/>
			</ReviewSection>

			<ReviewSection stepIndex={1}>
				<ReviewItem
					label={t(getLabelKey('grouping'))}
					value={
						state.grouping &&
						t(
							`${FIELDS_KEY}.queryBuilder.grouping.options.${state.grouping.toLowerCase()}.label`,
						)
					}
				/>
			</ReviewSection>

			<NewCard>
				<div className="px-4">
					<FilterConfigJSON />
				</div>
			</NewCard>
		</>
	)
}

// The labels the form's own selects show, not the raw enum values (PRIVATE, BY_SERIES)
const FIELDS_KEY = 'createOrUpdateSmartListForm.fields'
const LOCALE_KEY = 'createSmartListScene.form'
const getKey = (key: string) => `${LOCALE_KEY}.review.${key}`
const getLabelKey = (key: string) => getKey(`labels.${key}`)
