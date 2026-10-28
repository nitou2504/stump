import { zodResolver } from '@hookform/resolvers/zod'
import { useGraphQL } from '@stump/client'
import { CheckBox, Form } from '@stump/components'
import {
	FieldFilterString,
	graphql,
	MediaFilterFormQuery,
	MediaFilterInput,
	MediaMetadataFilterInput,
	ReadingStatus,
} from '@stump/graphql'
import { useEffect, useMemo, useState } from 'react'
import { FieldValues, useForm } from 'react-hook-form'
import z from 'zod'

import { useSeriesContextSafe } from '@/components/series/context'
import { useSmartListContextSafe } from '@/scenes/smartList/context'

import { useFilterContext } from '..'
import AgeRatingFilter from './AgeRatingFilter'
import ExtensionSelect from './ExtensionSelect'
import GenericFilterMultiselect from './GenericFilterMultiselect'
import ReadStatusSelect from './ReadStatusSelect'

const query = graphql(`
	query MediaFilterForm($seriesId: ID, $smartListId: ID) {
		mediaMetadataOverview(seriesId: $seriesId, smartListId: $smartListId) {
			genres
			languages
			writers
			pencillers
			colorists
			letterers
			inkers
			publishers
			editors
			characters
		}
		tags {
			id
			name
		}
	}
`)

type Overview = MediaFilterFormQuery['mediaMetadataOverview']

/**
 * The multi-select metadata fields of the form. `list` fields are stored as
 * comma-separated lists (e.g. `Novel, Science`) and are matched per element with
 * `hasAnyOf`, so `Science` doesn't also match `Science fiction`. The other fields hold a
 * single value and are matched exactly with `anyOf`.
 */
const METADATA_FIELDS = [
	{ name: 'genre', label: 'Genre', key: 'genres', overview: 'genres', list: true },
	{ name: 'writer', label: 'Writer', key: 'writers', overview: 'writers', list: true },
	{ name: 'language', label: 'Language', key: 'language', overview: 'languages', list: false },
	{ name: 'publisher', label: 'Publisher', key: 'publisher', overview: 'publishers', list: false },
	{
		name: 'penciller',
		label: 'Penciller',
		key: 'pencillers',
		overview: 'pencillers',
		list: true,
	},
	{ name: 'colorist', label: 'Colorist', key: 'colorists', overview: 'colorists', list: true },
	{ name: 'letterer', label: 'Letterer', key: 'letterers', overview: 'letterers', list: true },
	{ name: 'inker', label: 'Inker', key: 'inkers', overview: 'inkers', list: true },
	{ name: 'editor', label: 'Editor', key: 'editors', overview: 'editors', list: true },
	{
		name: 'character',
		label: 'Character',
		key: 'characters',
		overview: 'characters',
		list: true,
	},
] as const satisfies ReadonlyArray<{
	name: string
	label: string
	key: keyof MediaMetadataFilterInput
	overview: keyof Overview
	list: boolean
}>

type MetadataFieldName = (typeof METADATA_FIELDS)[number]['name']

const schema = z.object({
	extension: z.string().optional(),
	metadata: z
		.object({
			ageRating: z
				.number()
				.optional()
				.nullable()
				.refine((val) => val == null || (val >= 0 && val <= 18)),
			...(Object.fromEntries(
				METADATA_FIELDS.map(({ name }) => [name, z.array(z.string()).optional()]),
			) as Record<MetadataFieldName, z.ZodOptional<z.ZodArray<z.ZodString>>>),
		})
		.optional(),
	tags: z.array(z.string()).optional(),
	read_status: z.array(z.enum(['finished', 'reading', 'not_started'])).optional(),
})
export type MediaFilterFormSchema = z.infer<typeof schema>

/**
 * Reads the selected values back out of a string filter, accepting the legacy
 * substring operators so older links/bookmarks keep working
 */
const languageNames = (() => {
	try {
		return new Intl.DisplayNames([navigator.language, 'en'], { type: 'language' })
	} catch {
		return null
	}
})()

/** A human readable label for a two-letter language code, e.g. `es` -> `Spanish (es)` */
const languageLabel = (code: string) => {
	try {
		const name = languageNames?.of(code)
		return name && name !== code ? `${name} (${code})` : code
	} catch {
		return code
	}
}

const selectedValues = (filter?: FieldFilterString | null): string[] =>
	filter?.hasAnyOf ?? filter?.anyOf ?? filter?.likeAnyOf ?? []

export default function MediaFilterForm() {
	const { filters: filtersInput, setFilters } = useFilterContext()
	const filters = useMemo(() => (filtersInput || {}) as MediaFilterInput, [filtersInput])

	const seriesContext = useSeriesContextSafe()
	const [onlyFromSeries, setOnlyFromSeries] = useState(false)

	// Inside a smart list, only offer the values present in the list's books
	const smartListId = useSmartListContextSafe()?.list.id

	const params = useMemo(() => {
		if (smartListId) {
			return { smartListId }
		}
		if (onlyFromSeries && !!seriesContext?.series.id) {
			return {
				seriesId: seriesContext.series.id,
			}
		}
		return {}
	}, [onlyFromSeries, seriesContext, smartListId])

	const { data: _data, isPending } = useGraphQL(query, ['mediaFilterForm', params], params, {
		placeholderData: (prev) => prev,
	})
	const data = _data?.mediaMetadataOverview
	const tags = useMemo(() => _data?.tags ?? [], [_data])

	const defaultValue = useMemo(() => {
		const metadata = (filters?.metadata ?? {}) as MediaMetadataFilterInput
		const flattenMetadata = {
			ageRating: metadata.ageRating?.lte ?? null,
			...Object.fromEntries(
				METADATA_FIELDS.map(({ name, key }) => [
					name,
					selectedValues(metadata[key] as FieldFilterString | undefined),
				]),
			),
		}
		return {
			extension: filters?.extension?.eq as string,
			metadata: flattenMetadata,
			tags: selectedValues(filters?.tags),
			read_status: filters?.readingStatus?.isAnyOf?.map((elem) => (elem as string).toLowerCase()),
		}
	}, [filters])

	const form = useForm({
		defaultValues: defaultValue,
		resolver: zodResolver(schema),
	})
	const { reset } = form

	useEffect(() => {
		reset(defaultValue)
	}, [defaultValue, reset])

	/**
	 * A function that handles the form submission. This function merges the form
	 * values with the existing filters and sets the new filters.
	 * @param values The values from the form.
	 */
	const handleSubmit = (values: FieldValues) => {
		const newFilters: MediaFilterInput = {}
		if (values.extension) {
			newFilters.extension = { eq: values.extension }
		}

		if (values.read_status) {
			newFilters.readingStatus = {
				isAnyOf: values.read_status.map(
					(elem: string) => (elem as string).toUpperCase() as ReadingStatus,
				),
			}
		}

		if (values.tags?.length) {
			newFilters.tags = { anyOf: values.tags }
		}

		if (values.metadata) {
			const metadata: MediaMetadataFilterInput = {}
			if (values.metadata.ageRating != null && !Number.isNaN(values.metadata.ageRating)) {
				metadata.ageRating = { lte: values.metadata.ageRating }
			}

			for (const { name, key, list } of METADATA_FIELDS) {
				const selected = values.metadata[name] as string[] | undefined
				if (selected?.length) {
					Object.assign(metadata, {
						[key]: list ? { hasAnyOf: selected } : { anyOf: selected },
					})
				}
			}

			if (Object.keys(metadata).length) {
				newFilters.metadata = metadata
			}
		}

		setFilters(newFilters)
	}

	const isEmptyState = useMemo(() => !isPending && isEmptyResult(data), [isPending, data])

	return (
		<Form
			className="px-6 py-2 scrollbar-hide flex max-h-full grow flex-col overflow-x-visible overflow-y-auto"
			id="filter-form"
			form={form}
			onSubmit={handleSubmit}
		>
			{!!seriesContext && (
				<CheckBox
					label="Only show options available from series"
					checked={onlyFromSeries}
					onClick={() => setOnlyFromSeries((prev) => !prev)}
				/>
			)}

			<ExtensionSelect />
			<ReadStatusSelect />
			<AgeRatingFilter />

			{!isEmptyState &&
				METADATA_FIELDS.map(({ name, label, overview }) => {
					const options = (data?.[overview] ?? []).filter(Boolean)
					if (!options.length) return null
					return (
						<GenericFilterMultiselect
							key={name}
							name={`metadata.${name}`}
							label={label}
							options={options.map((value) => ({
								label: name === 'language' ? languageLabel(value) : value,
								value,
							}))}
						/>
					)
				})}

			{tags.length > 0 && (
				<GenericFilterMultiselect
					name="tags"
					label="Tags"
					options={tags.map(({ name }) => ({ label: name, value: name }))}
				/>
			)}
		</Form>
	)
}

const isEmptyResult = (result?: Overview) =>
	!(
		result?.genres?.length ||
		result?.languages?.length ||
		result?.writers?.length ||
		result?.pencillers?.length ||
		result?.colorists?.length ||
		result?.letterers?.length ||
		result?.inkers?.length ||
		result?.publishers?.length ||
		result?.editors?.length ||
		result?.characters?.length
	) || result == null
