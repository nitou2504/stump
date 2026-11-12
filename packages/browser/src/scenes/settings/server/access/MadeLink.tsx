import { Button, Input, Text } from '@stump/components'
import { Check, Copy, QrCode as QrIcon, Share2 } from 'lucide-react'
import { useState } from 'react'
import { toast } from 'sonner'

import { useTranslate } from '@/hooks/useTranslate'

import { copyText } from './copy'
import { accessKey } from './keys'
import QrCode from './QrCode'

/** What the owner is shown right after making a link: the one and only copy of its address */
export type MadeLinkValue = { url: string; label: string }

/**
 * A link that was just made, to send. It is shown here and nowhere else: Stump keeps a hash of the
 * token and can't show the link again, so a lost one means making another. *Copy* is the main
 * thing to do; *Share* is offered where the phone can share; the QR code (for another phone to
 * scan) is behind a button, so the address never sits on the page by accident
 */
export default function MadeLink({ made, onDone }: { made: MadeLinkValue; onDone: () => void }) {
	const { t } = useTranslate()
	const [copied, setCopied] = useState(false)
	const [qr, setQr] = useState(false)
	const canShare = typeof navigator !== 'undefined' && typeof navigator.share === 'function'

	const copy = async () => {
		const done = await copyText(made.url)
		setCopied(done)
		if (done) {
			toast.success(t(accessKey('invite.copied')))
		} else {
			// nothing could be copied: the link is selected in the box, to copy by hand
			toast.error(t(accessKey('invite.copyFailed')))
		}
	}

	const share = async () => {
		try {
			await navigator.share({ title: made.label, url: made.url })
		} catch {
			// closed without sharing, or refused: nothing to say
		}
	}

	return (
		<div data-testid="access-made-link" role="status" className="gap-4 flex flex-col">
			<Text className="font-medium">{t(accessKey('invite.sendTo'), { note: made.label })}</Text>
			<div className="gap-2 sm:flex-row flex flex-col">
				{canShare && (
					<Button variant="outline" className="sm:hidden" onClick={share}>
						<Share2 className="h-4 w-4" />
						{t(accessKey('invite.share'))}
					</Button>
				)}
				<Input
					aria-label={t(accessKey('invite.linkLabel'))}
					value={made.url}
					readOnly
					fullWidth
					onFocus={(event) => event.currentTarget.select()}
				/>
				<div className="gap-2 flex shrink-0">
					<Button onClick={copy}>
						{copied ? <Check className="h-4 w-4" /> : <Copy className="h-4 w-4" />}
						{t(accessKey('invite.copy'))}
					</Button>
					{canShare && (
						<Button variant="outline" className="max-sm:hidden" onClick={share}>
							<Share2 className="h-4 w-4" />
							{t(accessKey('invite.share'))}
						</Button>
					)}
				</div>
			</div>
			<div className="gap-3 flex flex-col items-start">
				<Button variant="ghost" size="sm" aria-expanded={qr} onClick={() => setQr((open) => !open)}>
					<QrIcon className="h-4 w-4" />
					{t(accessKey(qr ? 'invite.hideQr' : 'invite.showQr'))}
				</Button>
				{qr && <QrCode value={made.url} />}
			</div>
			<Text size="sm" variant="muted">
				{t(accessKey('invite.onlyNow'))}
			</Text>
			<div>
				<Button variant="outline" onClick={onDone}>
					{t(accessKey('invite.done'))}
				</Button>
			</div>
		</div>
	)
}
