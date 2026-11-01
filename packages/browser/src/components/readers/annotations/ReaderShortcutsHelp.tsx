import { Dialog, Text } from '@stump/components'
import { Fragment } from 'react'

import { useTranslate } from '@/hooks/useTranslate'

import { ANNOTATION_STYLES, BOOKMARK_SHORTCUT, HELP_SHORTCUT, NOTE_SHORTCUT } from './styles'

export type ShortcutRow = { keys: string[]; label: string }

type Props = {
	open: boolean
	onOpenChange: (open: boolean) => void
	/** Reader-specific shortcuts listed first, e.g. page turns and search */
	extra?: ShortcutRow[]
}

/** The reader's keyboard shortcuts, opened with `?` */
export default function ReaderShortcutsHelp({ open, onOpenChange, extra = [] }: Props) {
	const { translate } = useTranslate()

	const annotationRows: ShortcutRow[] = [
		...ANNOTATION_STYLES.map((meta) => ({
			keys: [meta.shortcut.toUpperCase()],
			label: translate(meta.labelKey),
		})),
		{ keys: [NOTE_SHORTCUT.toUpperCase()], label: translate('shared.readerAnnotations.addNote') },
		{ keys: ['Del'], label: translate('shared.readerAnnotations.shortcuts.deleteSelected') },
		{ keys: ['Esc'], label: translate('shared.readerAnnotations.shortcuts.dismiss') },
	]
	const generalRows: ShortcutRow[] = [
		...extra,
		{
			keys: [BOOKMARK_SHORTCUT.toUpperCase()],
			label: translate('shared.readerAnnotations.shortcuts.bookmark'),
		},
		{ keys: [HELP_SHORTCUT], label: translate('shared.readerAnnotations.shortcuts.help') },
	]

	return (
		<Dialog open={open} onOpenChange={onOpenChange}>
			<Dialog.Content size="sm">
				<Dialog.Header>
					<Dialog.Title>{translate('shared.readerAnnotations.shortcuts.title')}</Dialog.Title>
					<Dialog.Close onClick={() => onOpenChange(false)} />
				</Dialog.Header>

				<ShortcutTable rows={generalRows} />
				<Text size="xs" variant="muted" className="pt-1">
					{translate('shared.readerAnnotations.shortcuts.withSelection')}
				</Text>
				<ShortcutTable rows={annotationRows} />
			</Dialog.Content>
		</Dialog>
	)
}

function ShortcutTable({ rows }: { rows: ShortcutRow[] }) {
	return (
		<dl className="gap-x-4 gap-y-1.5 text-sm grid grid-cols-[6rem_1fr] items-center">
			{rows.map((row) => (
				<Fragment key={row.label}>
					<dt className="gap-1 flex">
						{row.keys.map((key) => (
							<kbd
								key={key}
								className="px-1.5 py-0.5 font-mono text-xs min-w-6 rounded border border-border bg-muted text-center"
							>
								{key}
							</kbd>
						))}
					</dt>
					<dd className="text-foreground">{row.label}</dd>
				</Fragment>
			))}
		</dl>
	)
}
