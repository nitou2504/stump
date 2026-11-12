import { Alert, AlertDescription, Button, Card, ConfirmationModal, Input, Text } from '@stump/components'
import { UserPermission } from '@stump/graphql'
import { Check, Copy, QrCode as QrIcon, RefreshCw, Trash2 } from 'lucide-react'
import { FormEvent, Suspense, useState } from 'react'
import { Helmet } from 'react-helmet'
import { toast } from 'sonner'

import { SceneContainer } from '@/components/container'
import { useAppContext } from '@/context'
import { useTranslate } from '@/hooks/useTranslate'
import { copyText } from '@/scenes/settings/server/access/copy'
import { formatWhen } from '@/scenes/settings/server/access/format'
import QrCode from '@/scenes/settings/server/access/QrCode'

import { type MyReadingApp, useMyAccessActions, useMyDevices, useMyReadingApps } from './graphql'

const key = (name: string) => `settingsScene.app/readingApps.${name}`

const errorMessage = (error: unknown) =>
	error instanceof Error && error.message ? error.message : undefined

type Pending = { kind: 'replace' | 'remove'; app: MyReadingApp }

function AppCard({ app, onAsk }: { app: MyReadingApp; onAsk: (pending: Pending) => void }) {
	const { t } = useTranslate()
	const [copied, setCopied] = useState(false)
	const [showQr, setShowQr] = useState(false)

	const copy = async () => {
		const done = await copyText(app.url)
		setCopied(done)
		if (done) toast.success(t(key('copied')))
		else toast.error(t(key('copyFailed')))
	}

	return (
		<Card data-testid={`reading-app-${app.id}`} className="gap-3 p-4 flex flex-col">
			<div className="gap-1 flex flex-col">
				<Text className="font-medium">{app.label}</Text>
				<Text size="xs" variant="muted">
					{t(key('lastUsed'), {
						at: app.lastSeenAt ? formatWhen(app.lastSeenAt) : t(key('never')),
					})}
				</Text>
			</div>

			<div className="gap-2 sm:flex-row flex flex-col">
				<Input
					aria-label={t(key('address'))}
					value={app.url}
					readOnly
					fullWidth
					onFocus={(event) => event.currentTarget.select()}
				/>
				<div className="gap-2 flex shrink-0">
					<Button onClick={copy}>
						{copied ? <Check className="h-4 w-4" /> : <Copy className="h-4 w-4" />}
						{t(key('copy'))}
					</Button>
					<Button variant="outline" aria-pressed={showQr} onClick={() => setShowQr((on) => !on)}>
						<QrIcon className="h-4 w-4" />
						{t(key('qr'))}
					</Button>
				</div>
			</div>

			{showQr && <QrCode value={app.url} />}

			<ol className="gap-1 pl-5 text-sm list-decimal flex flex-col text-muted-foreground">
				<li>{t(key('steps.0'))}</li>
				<li>{t(key('steps.1'))}</li>
				<li>{t(key('steps.2'))}</li>
			</ol>
			<Text size="xs" variant="muted">
				{t(key('otherReaders'))}
			</Text>

			<div className="gap-2 flex flex-wrap">
				<Button size="sm" variant="outline" onClick={() => onAsk({ kind: 'replace', app })}>
					<RefreshCw className="h-4 w-4" />
					{t(key('replace'))}
				</Button>
				<Button size="sm" variant="ghost" onClick={() => onAsk({ kind: 'remove', app })}>
					<Trash2 className="h-4 w-4" />
					{t(key('remove'))}
				</Button>
			</div>
		</Card>
	)
}

function AppsContent() {
	const { t } = useTranslate()
	const { checkPermission } = useAppContext()
	const apps = useMyReadingApps()
	const { active, limit } = useMyDevices()
	const actions = useMyAccessActions()
	const [name, setName] = useState('')
	const [pending, setPending] = useState<Pending | null>(null)

	const canDownload = checkPermission(UserPermission.DownloadFile)
	const atLimit = active >= limit
	const canAdd = canDownload && !atLimit && !!name.trim() && !actions.isPending

	const add = async (event: FormEvent) => {
		event.preventDefault()
		if (!canAdd) return
		try {
			await actions.createReadingApp(name.trim())
			setName('')
		} catch (error) {
			toast.error(t(key('errors.create')), { description: errorMessage(error) })
		}
	}

	const confirm = async () => {
		if (!pending) return
		try {
			if (pending.kind === 'replace') {
				await actions.replaceReadingApp(pending.app.id)
				toast.success(t(key('replaced'), { name: pending.app.label }))
			} else {
				await actions.removeReadingApp(pending.app.id)
				toast.success(t(key('removed'), { name: pending.app.label }))
			}
			setPending(null)
		} catch (error) {
			toast.error(t(key('errors.change')), { description: errorMessage(error) })
		}
	}

	return (
		<div className="gap-4 flex flex-col">
			<Text size="sm" variant="muted">
				{t(key('intro'))}
			</Text>

			{!canDownload && (
				<Alert variant="warning" data-testid="reading-apps-no-download">
					<AlertDescription>{t(key('noDownload'))}</AlertDescription>
				</Alert>
			)}
			{canDownload && atLimit && (
				<Alert variant="warning" data-testid="reading-apps-full">
					<AlertDescription>{t(key('full'), { limit })}</AlertDescription>
				</Alert>
			)}

			<form onSubmit={add} className="gap-2 sm:flex-row sm:items-end flex flex-col">
				<Input
					id="reading-app-name"
					label={t(key('name'))}
					placeholder={t(key('namePlaceholder'))}
					value={name}
					onChange={(event) => setName(event.target.value)}
					maxLength={40}
					disabled={!canDownload || atLimit}
					fullWidth
				/>
				<Button type="submit" disabled={!canAdd} isLoading={actions.isPending}>
					{t(key('add'))}
				</Button>
			</form>

			{apps.length === 0 && (
				<Text size="sm" variant="muted">
					{t(key('empty'))}
				</Text>
			)}
			<div className="gap-3 flex flex-col">
				{apps.map((app) => (
					<AppCard key={app.id} app={app} onAsk={setPending} />
				))}
			</div>

			<ConfirmationModal
				title={t(key(pending?.kind === 'replace' ? 'confirm.replaceTitle' : 'confirm.removeTitle'))}
				description={t(
					key(pending?.kind === 'replace' ? 'confirm.replaceDescription' : 'confirm.removeDescription'),
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
 * to yourself, and replace it if it leaks. Each one counts as one of your devices
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
