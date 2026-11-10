import { Label, NativeSelect, Text } from '@stump/components'
import { EntityVisibility } from '@stump/graphql'
import { useFormContext, useWatch } from 'react-hook-form'

import { useTranslate } from '@/hooks/useTranslate'

import { SmartListFormSchema } from '../schema'

type SubSchema = Pick<SmartListFormSchema, 'visibility'>

/**
 * Who sees the list: only its maker, or everyone on the server (each one with the books they may
 * read). There is no sharing with some people: the server doesn't offer `SHARED`
 */
export default function AccessSettings() {
	const form = useFormContext<SubSchema>()
	const visibility = useWatch({ control: form.control, name: 'visibility' })

	const { t } = useTranslate()

	return (
		<div className="max-w-xs gap-y-1.5 flex flex-col">
			<Label>{t(getKey('label'))}</Label>
			<NativeSelect
				options={[
					{ label: t(getOptionKey(EntityVisibility.Public, 'label')), value: 'PUBLIC' },
					{ label: t(getOptionKey(EntityVisibility.Private, 'label')), value: 'PRIVATE' },
				]}
				{...form.register('visibility')}
			/>
			<Text variant="muted" size="sm">
				{t(getOptionKey(visibility as EntityVisibility, 'description'))}
			</Text>
		</div>
	)
}

const LOCALE_KEY = 'createOrUpdateSmartListForm.fields.visibility'
const getKey = (key: string) => `${LOCALE_KEY}.${key}`
const getOptionKey = (option: EntityVisibility, key: string) =>
	getKey(`options.${option.toLowerCase()}.${key}`)
