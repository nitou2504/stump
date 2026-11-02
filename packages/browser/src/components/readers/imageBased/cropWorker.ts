/**
 * Finds a page's margins off the main thread, from a small copy of the page the reader
 * already decoded (see `pageAnalysis.ts`)
 */
import { pageCrop, sampleSize, toLuma } from './pageCrop'

export type CropRequest = {
	id: number
	bitmap: ImageBitmap
	natural: { w: number; h: number }
	withHalves: boolean
}

/** The worker's own scope (typed by hand: the project's types are the window's) */
const scope = self as unknown as {
	onmessage: ((event: MessageEvent<CropRequest>) => void) | null
	postMessage: (message: unknown) => void
}

scope.onmessage = (event: MessageEvent<CropRequest>) => {
	const { id, bitmap, natural, withHalves } = event.data
	try {
		const sample = sampleSize(natural.w, natural.h)
		const canvas = new OffscreenCanvas(sample.w, sample.h)
		const context = canvas.getContext('2d', { willReadFrequently: true })
		if (!context) throw new Error('no 2d context')
		context.fillStyle = '#fff'
		context.fillRect(0, 0, sample.w, sample.h)
		context.drawImage(bitmap, 0, 0, sample.w, sample.h)
		bitmap.close()
		const pixels = context.getImageData(0, 0, sample.w, sample.h).data
		const luma = toLuma(pixels, sample.w * sample.h)
		scope.postMessage({ id, crop: pageCrop(luma, sample, natural, withHalves) })
	} catch (error) {
		bitmap.close?.()
		scope.postMessage({ id, error: String(error) })
	}
}
