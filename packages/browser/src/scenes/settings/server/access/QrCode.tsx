import { lazy, Suspense } from 'react'

import { useTranslate } from '@/hooks/useTranslate'

import { accessKey } from './keys'

// `qrcode.react` is only fetched when a QR code is drawn: the page of links and the Reading apps page
const QRCodeSVG = lazy(() => import('qrcode.react').then((module) => ({ default: module.QRCodeSVG })))

type Props = {
	/** What the code holds: it is drawn in the browser and never leaves the page */
	value: string
	/** The side in pixels */
	size?: number
}

/**
 * A QR code of `value`, for a phone to scan. It is made here, in the page, from the text it is
 * given: the address of a link or of a reading app is not sent anywhere to make it. It is drawn on
 * white whatever the theme (a scanner needs the contrast)
 */
export default function QrCode({ value, size = 168 }: Props) {
	const { t } = useTranslate()

	return (
		<div
			data-testid="access-qr"
			role="img"
			aria-label={t(accessKey('qr.label'))}
			className="rounded-lg bg-white p-3 inline-block"
			style={{ width: size + 24, height: size + 24 }}
		>
			<Suspense fallback={null}>
				<QRCodeSVG value={value} size={size} level="M" bgColor="#ffffff" fgColor="#000000" />
			</Suspense>
		</div>
	)
}
