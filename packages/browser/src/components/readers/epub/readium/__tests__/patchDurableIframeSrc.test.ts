import type { EpubNavigator } from '@readium/navigator'

import { recoverBlankFrames } from '../patchDurableIframeSrc'

/** A frame of a fixed-layout book's pool (Readium's FXL frame manager), not loaded yet */
function blankFixedLayoutFrame() {
	return {
		iframe: {
			contentWindow: { location: { href: 'about:blank' } },
			style: { visibility: 'hidden' },
		},
		currModules: ['snapper'],
		// FXL frames load with the page's source too; called without it they went to `/undefined`
		load: vi.fn(),
		show: vi.fn(),
	}
}

describe('recoverBlankFrames', () => {
	it("leaves a fixed-layout book's frames to Readium", async () => {
		const frame = blankFixedLayoutFrame()
		const navigator = {
			_cframes: [frame],
			pool: { pool: new Map([['page001.xhtml', frame]]) },
		} as unknown as EpubNavigator

		expect(await recoverBlankFrames(navigator)).toBe(0)
		expect(frame.load).not.toHaveBeenCalled()
	})
})
