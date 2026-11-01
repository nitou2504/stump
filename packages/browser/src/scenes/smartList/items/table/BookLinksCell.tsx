import { Badge } from '@stump/components'
import { ExternalLink } from 'lucide-react'

type Props = {
	links: string[]
}

export default function BookLinksCell({ links }: Props) {
	const getBaseURL = (href: string) => {
		try {
			const url = new URL(href)

			return url.host
		} catch {
			return href
		}
	}

	if (!links.length) {
		return null
	}

	// A single line, like every other cell: the links that don't fit are clipped
	return (
		<div
			className="gap-1.5 min-w-0 flex flex-nowrap items-center overflow-hidden"
			title={links.join('\n')}
		>
			{links.map((href) => (
				<a key={href} href={href} target="_blank" rel="noopener noreferrer" className="shrink-0">
					<Badge size="sm">
						{getBaseURL(href)}
						<ExternalLink className="ml-1 h-3 w-3" />
					</Badge>
				</a>
			))}
		</div>
	)
}
