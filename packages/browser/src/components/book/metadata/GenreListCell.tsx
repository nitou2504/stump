import { Badge } from '@stump/components'
import { MetadataFacetField } from '@stump/graphql'
import { useFormContext, useWatch } from 'react-hook-form'
import { Link } from 'react-router'

import { useMetadataEditorContext } from '@/components/metadata/metadataEditor/context'

import ValueChipsField from './edit/ValueChipsField'

type Props = {
	field: 'genres' | 'writers'
	values: string[]
	itemUrl: (item: string) => string
}

/**
 * The genres or authors row of the metadata table: links when reading, and while editing
 * chips with a picker over the values the library already uses (a new genre needs an
 * explicit "Create", so a typo doesn't start a new shelf)
 */
export default function GenreListCell({ field, values, itemUrl }: Props) {
	const form = useFormContext()
	const { isEditing, isFieldLocked } = useMetadataEditorContext()
	const editing = isEditing && !isFieldLocked(field)
	const fromForm: string[] | null | undefined = useWatch({ control: form.control, name: field })

	if (editing) {
		return (
			<ValueChipsField
				field={field === 'genres' ? MetadataFacetField.Genres : MetadataFacetField.Writers}
				values={fromForm ?? []}
				onChange={(next) => form.setValue(field, next, { shouldDirty: true })}
				addLabel={field === 'genres' ? 'Add a genre' : 'Add an author'}
			/>
		)
	}

	return (
		<div className="gap-1.5 flex h-full flex-wrap items-center">
			{values.map((value, index) => (
				<Link key={`${value}-${index}`} to={itemUrl(value)}>
					<Badge className="cursor-pointer">{value}</Badge>
				</Link>
			))}
		</div>
	)
}
