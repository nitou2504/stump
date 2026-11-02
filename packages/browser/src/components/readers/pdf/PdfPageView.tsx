import { cn } from '@stump/components'
import { memo, useCallback, useEffect, useState } from 'react'

import { useTranslate } from '@/hooks/useTranslate'

import type { ReaderAnnotation } from '../annotations'
import type { FractionRect, Size } from './geometry'
import PdfAnnotationLayer from './PdfAnnotationLayer'
import PdfTextLayer from './PdfTextLayer'
import type { PdfThemeLook } from './pdfThemes'
import { PAGE_ATTRIBUTE } from './selection'
import { usePdfPageText } from './usePdfData'

type Props = {
	mediaId: string
	/** 1-based */
	page: number
	/** The layout's document version */
	v: string
	size: Size
	/** The page's width / height in points */
	aspect: number
	/** The picture URL to show (already sized for the screen) */
	src: string
	/**
	 * Whether the picture and text may load now: pages around the view wait until the
	 * pages in view are up
	 */
	canLoad: boolean
	onPictureLoaded: (src: string) => void
	/** Whether the PDF has a text layer to fetch */
	hasText: boolean
	annotations: ReaderAnnotation[]
	look: PdfThemeLook
	selectedAnnotationId?: string | null
	flashRects?: FractionRect[] | null
	onOpenAnnotation: (annotationId: string, anchor: DOMRect) => void
	/** Position in the scrolling content (numbers, so the memo holds while scrolling) */
	top: number
	left: number
	className?: string
}

/**
 * One PDF page: its picture (the server renders it at the width the screen needs), the
 * selectable text over it and the user's annotations. While a sharper picture for a new
 * zoom loads, the previous one stays up, stretched, so the page never goes blank.
 */
function PdfPageView({
	mediaId,
	page,
	v,
	size,
	aspect,
	src,
	canLoad,
	onPictureLoaded,
	hasText,
	annotations,
	look,
	selectedAnnotationId,
	flashRects,
	onOpenAnnotation,
	top,
	left,
	className,
}: Props) {
	const { translate } = useTranslate()
	const [shownSrc, setShownSrc] = useState<string | null>(null)
	const [loaded, setLoaded] = useState(false)
	const [failed, setFailed] = useState(false)
	const [retry, setRetry] = useState(0)
	const [textReady, setTextReady] = useState(false)

	const wantedSrc = retry ? `${src}${src.includes('?') ? '&' : '?'}retry=${retry}` : src

	// Swap to a new picture (another zoom tier) only once it has decoded
	useEffect(() => {
		if (!shownSrc || shownSrc === wantedSrc) return undefined
		let cancelled = false
		const image = new Image()
		image.decoding = 'async'
		image.src = wantedSrc
		image
			.decode()
			.catch(() => undefined)
			.then(() => {
				if (!cancelled) setShownSrc(wantedSrc)
			})
		return () => {
			cancelled = true
		}
	}, [shownSrc, wantedSrc])

	const displayedSrc = shownSrc ?? wantedSrc

	const handleLoad = useCallback(() => {
		setLoaded(true)
		setFailed(false)
		setShownSrc((current) => current ?? displayedSrc)
		onPictureLoaded(src)
	}, [displayedSrc, onPictureLoaded, src])

	// once a picture has been shown, later ones (other zoom tiers) load regardless
	const mayLoad = canLoad || !!shownSrc

	// Build the text layer after the picture is up, when the browser is idle, so it never
	// delays a page turn
	const { data: text } = usePdfPageText(mediaId, v, page, hasText && mayLoad)
	useEffect(() => {
		if (!loaded || !text || textReady) return undefined
		const idle = window.requestIdleCallback
		if (idle) {
			const handle = idle(() => setTextReady(true), { timeout: 400 })
			return () => window.cancelIdleCallback?.(handle)
		}
		const timeout = window.setTimeout(() => setTextReady(true), 50)
		return () => window.clearTimeout(timeout)
	}, [loaded, text, textReady])

	return (
		<div
			{...{ [PAGE_ATTRIBUTE]: page }}
			className={cn('pdf-page', className)}
			style={{
				top,
				left,
				width: size.width,
				height: size.height,
				backgroundColor: look.pagePlaceholder,
			}}
			// Build the text layer right away when the user starts selecting
			onPointerDown={() => !textReady && text && setTextReady(true)}
		>
			{!failed && mayLoad && (
				<img
					src={displayedSrc}
					alt=""
					draggable={false}
					decoding="async"
					className="pdf-page-image"
					style={{ filter: look.imageFilter }}
					onLoad={handleLoad}
					onError={() => {
						setFailed(true)
						// don't hold the other pages back
						onPictureLoaded(src)
					}}
				/>
			)}
			{look.tint && <div className="pdf-page-tint" style={{ backgroundColor: look.tint }} />}
			{failed && (
				<button
					type="button"
					className="pdf-page-error"
					onClick={() => {
						setFailed(false)
						setRetry((value) => value + 1)
					}}
				>
					{translate('shared.pdfReader.pageFailed', { page })}
				</button>
			)}
			<PdfAnnotationLayer
				annotations={annotations}
				look={look}
				selectedId={selectedAnnotationId}
				flashRects={flashRects}
				onOpenAnnotation={onOpenAnnotation}
			/>
			{textReady && text && text.runs.length > 0 && (
				<PdfTextLayer runs={text.runs} aspect={aspect} pageHeight={size.height} />
			)}
		</div>
	)
}

export default memo(PdfPageView)
