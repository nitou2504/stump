import { BookOpen, FileText, Images, Layers } from 'lucide-react'

/** The icon of a book's format (a series of volumes has its own) */
export function FormatIcon({
	format,
	series,
	className,
}: {
	format?: string
	series?: boolean
	className?: string
}) {
	const Icon = series
		? Layers
		: format === 'pdf'
			? FileText
			: format === 'cbz' || format === 'cbr'
				? Images
				: BookOpen
	return <Icon className={className} aria-hidden />
}

/** `epub` → `EPUB`: how a format reads on a chip */
export const formatLabel = (format?: string) => (format ? format.toUpperCase() : undefined)
