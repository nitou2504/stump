import { Dialog, Text } from '@stump/components'
import { Fragment } from 'react'

import { useTranslate } from '@/hooks/useTranslate'

import { COPY_SHORTCUT, DAY_NIGHT_SHORTCUT, SEARCH_SHORTCUT_KEYS } from '../shared/readerKeys'
import { ANNOTATION_STYLES, BOOKMARK_SHORTCUT, HELP_SHORTCUT, NOTE_SHORTCUT } from './styles'

export type ShortcutRow = { keys: string[]; label: string }

type Translate = (key: string) => string

/**
 * Every reader's keyboard shortcuts: the reader's own (`extra`, e.g. page turns and zoom)
 * and the shared ones, then those that act on selected text or an annotation
 */
export function readerShortcutRows(
	translate: Translate,
	extra: ShortcutRow[] = [],
): { general: ShortcutRow[]; withSelection: ShortcutRow[] } {
	return {
		general: [
			...extra,
			{
				keys: SEARCH_SHORTCUT_KEYS,
				label: translate('shared.readerAnnotations.shortcuts.search'),
			},
			{
				keys: [BOOKMARK_SHORTCUT.toUpperCase()],
				label: translate('shared.readerAnnotations.shortcuts.bookmark'),
			},
			{
				keys: [DAY_NIGHT_SHORTCUT.toUpperCase()],
				label: translate('shared.reader.shortcuts.dayNight'),
			},
			{ keys: [HELP_SHORTCUT], label: translate('shared.readerAnnotations.shortcuts.help') },
		],
		withSelection: [
			...ANNOTATION_STYLES.map((meta) => ({
				keys: [meta.shortcut.toUpperCase()],
				label: translate(meta.labelKey),
			})),
			{ keys: [NOTE_SHORTCUT.toUpperCase()], label: translate('shared.readerAnnotations.addNote') },
			{ keys: [COPY_SHORTCUT.toUpperCase()], label: translate('shared.readerAnnotations.copy') },
			{ keys: ['Del'], label: translate('shared.readerAnnotations.shortcuts.deleteSelected') },
			{ keys: ['Esc'], label: translate('shared.readerAnnotations.shortcuts.dismiss') },
		],
	}
}

/** The shortcuts as a list (the reader settings show it in their last section) */
export function ReaderShortcutsList({ extra = [] }: { extra?: ShortcutRow[] }) {
	const { translate } = useTranslate()
	const { general, withSelection } = readerShortcutRows(translate, extra)
	return (
		<div className="gap-2 flex flex-col">
			<ShortcutTable rows={general} />
			<Text size="xs" variant="muted" className="pt-1">
				{translate('shared.readerAnnotations.shortcuts.withSelection')}
			</Text>
			<ShortcutTable rows={withSelection} />
		</div>
	)
}

type Props = {
	open: boolean
	onOpenChange: (open: boolean) => void
	/** Reader-specific shortcuts listed first, e.g. page turns and search */
	extra?: ShortcutRow[]
}

/** The reader's keyboard shortcuts in a dialog of their own */
export default function ReaderShortcutsHelp({ open, onOpenChange, extra = [] }: Props) {
	const { translate } = useTranslate()

	return (
		<Dialog open={open} onOpenChange={onOpenChange}>
			<Dialog.Content size="sm">
				<Dialog.Header>
					<Dialog.Title>{translate('shared.readerAnnotations.shortcuts.title')}</Dialog.Title>
					<Dialog.Close onClick={() => onOpenChange(false)} />
				</Dialog.Header>
				<ReaderShortcutsList extra={extra} />
			</Dialog.Content>
		</Dialog>
	)
}

export function ShortcutTable({ rows }: { rows: ShortcutRow[] }) {
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
