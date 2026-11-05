import { formatBytesSeparate } from '@stump/client'
import { MiniStatCard } from '@stump/components'
import { FragmentType, graphql, useFragment } from '@stump/graphql'
import { useLocaleContext } from '@stump/i18n'
import {
	Calendar,
	FileStack,
	FileType,
	HardDrive,
	Languages,
	type LucideIcon,
	ShieldAlert,
	Star,
} from 'lucide-react'

import { useTranslate } from '@/hooks/useTranslate'
import { bookLanguage } from '@/utils/language'

const fragment = graphql(`
	fragment BookStatRow on Media {
		pages
		size
		extension
		metadata {
			year
			rating
			ageRating
			language
		}
	}
`)

type Props = {
	fragment: FragmentType<typeof fragment>
}

export default function BookStatRow({ fragment: fragmentRef }: Props) {
	const { translate } = useTranslate()
	const { locale } = useLocaleContext()
	const { pages, size, extension, metadata } = useFragment(fragment, fragmentRef)
	const formattedSize = size > 0 ? formatBytesSeparate(size) : null

	const rating = metadata?.rating != null ? Number(metadata.rating) : null
	const language = bookLanguage(metadata?.language, locale)
	const items: {
		key: string
		icon: LucideIcon
		value?: string | number | null
		suffix?: string
		title?: string
	}[] = [
		{
			key: 'rating',
			icon: Star,
			value: rating != null && Number.isFinite(rating) && rating > 0 ? rating.toFixed(1) : null,
			suffix: '/ 5',
		},
		{
			key: 'pages',
			icon: FileStack,
			value: pages > 0 ? pages : null,
			suffix: translate('shared.bookMetadata.pageCount').toLowerCase(),
		},
		{
			key: 'year',
			icon: Calendar,
			value: metadata?.year && metadata.year > 0 ? metadata.year : null,
		},
		{ key: 'format', icon: FileType, value: extension?.toUpperCase() },
		{ key: 'language', icon: Languages, value: language?.label, title: language?.name },
		{
			key: 'size',
			icon: HardDrive,
			value: formattedSize?.value,
			suffix: formattedSize?.unit,
		},
		{
			key: 'age',
			icon: ShieldAlert,
			value: metadata?.ageRating && metadata.ageRating > 0 ? `${metadata.ageRating}+` : null,
		},
	].filter(({ value }) => value != null && value !== '')

	if (!items.length) return null

	return (
		<div className="gap-2 flex flex-wrap items-center">
			{items.map(({ key, icon, value, suffix, title }) => (
				<MiniStatCard key={key} icon={icon} value={value!} suffix={suffix} title={title} />
			))}
		</div>
	)
}
