import {
	Alert,
	AlertDescription,
	Button,
	ButtonOrLink,
	ConfirmationModal,
	DropdownMenu,
	Input,
	NewCard,
	Text,
} from '@stump/components'
import { UserPermission } from '@stump/graphql'
import { BookOpen, Check, Copy, MoreHorizontal, Plus, QrCode as QrIcon } from 'lucide-react'
import { FormEvent, Suspense, useState } from 'react'
import { Helmet } from 'react-helmet'
import { toast } from 'sonner'

import { SceneContainer } from '@/components/container'
import { useAppContext } from '@/context'
import { useTranslate } from '@/hooks/useTranslate'
import { usePaths } from '@/paths'
import { copyText } from '@/scenes/settings/server/access/copy'
import { formatAgo, formatWhen } from '@/scenes/settings/server/access/format'
import ModalPanel from '@/scenes/settings/server/access/ModalPanel'
import QrCode from '@/scenes/settings/server/access/QrCode'

import { type MyReadingApp, useMyAccessActions, useMyDevices, useMyReadingApps } from './graphql'

const key = (name: string) => `settingsScene.app/readingApps.${name}`

const errorMessage = (error: unknown) =>
	error instanceof Error && error.message ? error.message : undefined

type Pending = { kind: 'replace' | 'remove'; app: MyReadingApp }

/** What the dialog is showing: the question for a name, or the address of an app with what to do with it */
type Dialog = { step: 'add' } | { step: 'address'; label: string; url: string } | null

/** The address of an app, with how to use it. It is only on screen here, never in the list */
function AddressStep({ label, url, onDone }: { label: string; url: string; onDone: () => void }) {
	const { t } = useTranslate()
	const [copied, setCopied] = useState(false)
	const [showQr, setShowQr] = useState(false)

	const copy = async () => {
		const done = await copyText(url)
		setCopied(done)
		if (done) toast.success(t(key('copied')))
		else toast.error(t(key('copyFailed')))
	}

	return (
		<div data-testid="reading-app-address" className="gap-4 flex flex-col">
			<Text className="font-medium">{t(key('addressFor'), { name: label })}</Text>
			<div className="gap-2 sm:flex-row flex flex-col">
				<Input
					aria-label={t(key('address'))}
					value={url}
					readOnly
					fullWidth
					onFocus={(event) => event.currentTarget.select()}
				/>
				<Button onClick={copy} className="shrink-0">
					{copied ? <Check className="h-4 w-4" /> : <Copy className="h-4 w-4" />}
					{t(key('copy'))}
				</Button>
			</div>
			<div className="gap-3 flex flex-col items-start">
				<Button
					variant="ghost"
					size="sm"
					aria-expanded={showQr}
					onClick={() => setShowQr((on) => !on)}
				>
					<QrIcon className="h-4 w-4" />
					{t(key(showQr ? 'hideQr' : 'showQr'))}
				</Button>
				{showQr && <QrCode value={url} />}
			</div>
			<ol className="gap-1 pl-5 text-sm flex list-decimal flex-col text-muted-foreground">
				<li>{t(key('steps.0'))}</li>
				<li>{t(key('steps.1'))}</li>
				<li>{t(key('steps.2'))}</li>
			</ol>
			<Text size="sm" variant="muted">
				{t(key('keepPrivate'))}
			</Text>
			<div>
				<Button variant="outline" onClick={onDone}>
					{t(key('done'))}
				</Button>
			</div>
		</div>
	)
}

/** The name of the new app, and then its address */
function AddBody({
	dialog,
	onShow,
	onClose,
}: {
	dialog: Exclude<Dialog, null>
	onShow: (label: string, url: string) => void
	onClose: () => void
}) {
	const { t } = useTranslate()
	const actions = useMyAccessActions()
	const [name, setName] = useState('')

	const add = async (event: FormEvent) => {
		event.preventDefault()
		if (!name.trim()) return
		try {
			const app = await actions.createReadingApp(name.trim())
			onShow(app.label, app.url)
		} catch (error) {
			toast.error(t(key('errors.create')), { description: errorMessage(error) })
		}
	}

	if (dialog.step === 'address') {
		return <AddressStep label={dialog.label} url={dialog.url} onDone={onClose} />
	}

	return (
		<form onSubmit={add} className="gap-4 flex flex-col" data-testid="reading-app-form">
			<Input
				id="reading-app-name"
				label={t(key('name'))}
				placeholder={t(key('namePlaceholder'))}
				value={name}
				onChange={(event) => setName(event.target.value)}
				maxLength={40}
				autoFocus
				fullWidth
			/>
			<div className="gap-2 flex">
				<Button type="button" variant="outline" onClick={onClose}>
					{t(key('cancel'))}
				</Button>
				<Button type="submit" disabled={!name.trim()} isLoading={actions.isPending}>
					{t(key('addConfirm'))}
				</Button>
			</div>
		</form>
	)
}

function AppRow({
	app,
	onShow,
	onAsk,
}: {
	app: MyReadingApp
	onShow: (app: MyReadingApp) => void
	onAsk: (pending: Pending) => void
}) {
	const { t } = useTranslate()

	return (
		<div data-testid={`reading-app-${app.id}`} className="gap-3 px-4 py-3 flex items-center">
			<BookOpen aria-hidden className="h-4 w-4 shrink-0 text-muted-foreground" />
			<div className="min-w-0 flex-1">
				<Text size="sm" className="font-medium break-words">
					{app.label}
				</Text>
				<Text size="xs" variant="muted" title={formatWhen(app.lastSeenAt)}>
					{app.lastSeenAt
						? t(key('used'), { when: formatAgo(app.lastSeenAt) })
						: t(key('neverUsed'))}
				</Text>
			</div>
			<Button size="sm" variant="outline" onClick={() => onShow(app)}>
				{t(key('showAddress'))}
			</Button>
			<DropdownMenu
				align="end"
				trigger={
					<Button
						variant="ghost"
						size="icon"
						className="shrink-0"
						aria-label={t(key('moreActions'), { name: app.label })}
					>
						<MoreHorizontal className="h-4 w-4" />
					</Button>
				}
				groups={[
					{
						items: [
							{ label: t(key('replace')), onClick: () => onAsk({ kind: 'replace', app }) },
							{
								label: t(key('remove')),
								variant: 'destructive',
								onClick: () => onAsk({ kind: 'remove', app }),
							},
						],
					},
				]}
			/>
		</div>
	)
}

function AppsContent() {
	const { t } = useTranslate()
	const paths = usePaths()
	const { checkPermission } = useAppContext()
	const apps = useMyReadingApps()
	const { active, limit } = useMyDevices()
	const actions = useMyAccessActions()
	const [dialog, setDialog] = useState<Dialog>(null)
	const [pending, setPending] = useState<Pending | null>(null)

	const canDownload = checkPermission(UserPermission.DownloadFile)
	const atLimit = active >= limit

	const confirm = async () => {
		if (!pending) return
		const asked = pending
		try {
			if (asked.kind === 'replace') {
				const app = await actions.replaceReadingApp(asked.app.id)
				toast.success(t(key('replaced'), { name: asked.app.label }))
				setPending(null)
				// the new address is the point of replacing it: show it, once the question is gone
				// (two dialogs must not be open at once)
				setTimeout(() => setDialog({ step: 'address', label: asked.app.label, url: app.url }), 0)
			} else {
				await actions.removeReadingApp(asked.app.id)
				toast.success(t(key('removed'), { name: asked.app.label }))
				setPending(null)
			}
		} catch (error) {
			toast.error(t(key('errors.change')), { description: errorMessage(error) })
		}
	}

	return (
		<div className="gap-4 flex flex-col">
			<div className="gap-3 sm:flex-row sm:items-center flex flex-col justify-between">
				<Text size="sm" variant="muted">
					{t(key('intro'))}
				</Text>
				<Button
					onClick={() => setDialog({ step: 'add' })}
					disabled={!canDownload || atLimit}
					className="max-sm:w-full shrink-0"
				>
					<Plus className="h-4 w-4" />
					{t(key('add'))}
				</Button>
			</div>

			{!canDownload && (
				<Alert variant="warning" data-testid="reading-apps-no-download">
					<AlertDescription>{t(key('noDownload'))}</AlertDescription>
				</Alert>
			)}
			{canDownload && atLimit && (
				<Alert variant="warning" data-testid="reading-apps-full">
					<AlertDescription className="gap-2 flex flex-wrap items-center">
						{t(key('full'), { limit, page: t('settingsScene.app/devices.title') })}
						<ButtonOrLink variant="outline" size="sm" href={paths.settings('devices')}>
							{t(key('myDevices'))}
						</ButtonOrLink>
					</AlertDescription>
				</Alert>
			)}

			{apps.length === 0 ? (
				<Text size="sm" variant="muted">
					{t(key('empty'))}
				</Text>
			) : (
				<NewCard>
					<div className="divide-y divide-border/70">
						{apps.map((app) => (
							<AppRow
								key={app.id}
								app={app}
								onShow={(shown) =>
									setDialog({ step: 'address', label: shown.label, url: shown.url })
								}
								onAsk={setPending}
							/>
						))}
					</div>
				</NewCard>
			)}

			<ModalPanel
				open={!!dialog}
				onClose={() => setDialog(null)}
				title={t(key(dialog?.step === 'address' ? 'addressTitle' : 'addTitle'))}
				testId="reading-app-dialog"
			>
				{dialog && (
					<AddBody
						dialog={dialog}
						onShow={(label, url) => setDialog({ step: 'address', label, url })}
						onClose={() => setDialog(null)}
					/>
				)}
			</ModalPanel>

			<ConfirmationModal
				title={t(key(pending?.kind === 'replace' ? 'confirm.replaceTitle' : 'confirm.removeTitle'))}
				description={t(
					key(
						pending?.kind === 'replace'
							? 'confirm.replaceDescription'
							: 'confirm.removeDescription',
					),
					{ name: pending?.app.label ?? '' },
				)}
				isOpen={!!pending}
				onClose={() => setPending(null)}
				onConfirm={confirm}
				confirmIsLoading={actions.isPending}
				confirmText={t(key(pending?.kind === 'replace' ? 'replace' : 'remove'))}
				confirmVariant="destructive"
			/>
		</div>
	)
}

/**
 * *Reading apps*: the address to give KOReader, a Kobo or another OPDS reader. The key is part of
 * the address, so no sign-in is needed in the app (leave its username and password empty): keep it
 * to yourself, and replace it if it leaks. Each one counts as one of your devices. The page is a
 * list and one button; an address (and its QR code) shows only when an app was just added or when
 * you ask for it
 */
export default function ReadingAppsScene() {
	const { t } = useTranslate()

	return (
		<SceneContainer variant="settings">
			<Helmet>
				<title>Stump | {t(key('helmet'))}</title>
			</Helmet>
			<Suspense fallback={null}>
				<AppsContent />
			</Suspense>
		</SceneContainer>
	)
}
