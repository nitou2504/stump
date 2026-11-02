type FrameLike =
	| {
			iframe?: { contentWindow?: Window | null } | null
	  }
	| null
	| undefined

/**
 * Walks a Readium navigator's frame pool and clears any active text selection inside
 * each frame's content window. Extracted as a pure helper so the navigator's private
 * `_cframes` contract (see `useReadiumNavigator`'s `clearSelection`) can be unit tested
 * without mounting the full navigator.
 */
export function clearFramesSelection(frames: Iterable<FrameLike>): void {
	for (const frame of frames) {
		try {
			const win = frame?.iframe?.contentWindow
			win?.getSelection()?.removeAllRanges()
		} catch {
			// Cross-origin or destroyed frame, ignore.
		}
	}
}

type FrameWithComms =
	| {
			msg?: { send: (key: string, data: unknown, callback?: (ok: boolean) => void) => void } | null
	  }
	| null
	| undefined

/**
 * After the view was moved by hand (`scrollLeft`, see `edgeExtend`), lets each frame's
 * paginator snap to its page again and report where it is, as when the frame is shown
 * (Readium's `focus` message). The same private-frame contract as `clearFramesSelection`.
 */
export function resyncFramesPage(frames: Iterable<FrameWithComms>): void {
	for (const frame of frames) {
		try {
			frame?.msg?.send('focus', undefined, () => {})
		} catch {
			// A destroyed frame, ignore.
		}
	}
}
