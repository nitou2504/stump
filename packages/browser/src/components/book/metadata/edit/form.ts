import type { MediaMetadataInput } from '@stump/graphql'

import { orderTags } from './tags'

/** The fields of the "Edit details" dialog, as typed */
export type DetailsForm = {
	title: string
	writers: string[]
	series: string
	number: string
	genres: string[]
	summary: string
	publisher: string
	year: string
	language: string
	ageRating: string
}

type StoredDetails = {
	title?: string | null
	writers?: string[] | null
	series?: string | null
	number?: unknown
	genres?: string[] | null
	summary?: string | null
	publisher?: string | null
	year?: number | null
	language?: string | null
	ageRating?: number | null
}

const text = (value: unknown) => (value == null ? '' : String(value))

/** The form for a book's stored details */
export function detailsForm(meta?: StoredDetails | null): DetailsForm {
	return {
		ageRating: text(meta?.ageRating),
		genres: meta?.genres ?? [],
		language: text(meta?.language),
		number: text(meta?.number),
		publisher: text(meta?.publisher),
		series: text(meta?.series),
		summary: text(meta?.summary),
		title: text(meta?.title),
		writers: meta?.writers ?? [],
		year: text(meta?.year),
	}
}

/** The text of a field, or null when empty */
const orNull = (value: string) => (value.trim() ? value.trim() : null)

const toInt = (value: string) => {
	const n = Number.parseInt(value, 10)
	return Number.isFinite(n) ? n : null
}

const toNumber = (value: string) => {
	const n = Number.parseFloat(value.replace(',', '.'))
	return Number.isFinite(n) ? n : null
}

const sameList = (a: string[], b: string[]) =>
	a.length === b.length && a.every((value, i) => value === b[i])

/**
 * What a save sends: only the fields that differ from what was loaded, so a save never
 * rewrites (and locks) a field nobody touched. A new year clears the month and day it
 * came with; clearing the series clears its number.
 */
export function editInput(initial: DetailsForm, form: DetailsForm): MediaMetadataInput {
	const input: MediaMetadataInput = {}
	const changed = (key: 'title' | 'series' | 'number' | 'publisher' | 'year' | 'language') =>
		form[key].trim() !== initial[key].trim()

	if (changed('title')) input.title = orNull(form.title)
	if (!sameList(form.writers, initial.writers)) input.writers = form.writers
	if (changed('series')) {
		input.series = orNull(form.series)
		if (!orNull(form.series) && initial.number.trim()) input.number = null
	}
	if (changed('number') && orNull(form.series)) input.number = toNumber(form.number)
	if (!sameList(form.genres, initial.genres)) input.genres = orderTags(form.genres)
	if (form.summary !== initial.summary) input.summary = orNull(form.summary)
	if (changed('publisher')) input.publisher = orNull(form.publisher)
	if (changed('year')) {
		input.year = toInt(form.year)
		input.month = null
		input.day = null
	}
	if (changed('language')) input.language = orNull(form.language)
	if (form.ageRating !== initial.ageRating) input.ageRating = toInt(form.ageRating)
	return input
}
