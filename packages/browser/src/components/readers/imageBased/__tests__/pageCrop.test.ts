import { contentBox, pageCrop, sampleSize, scaleBox, toLuma } from '../pageCrop'

/** A grey image `w` × `h` of paper (255) with a dark rectangle of ink inside */
function page(
	w: number,
	h: number,
	ink: { x: number; y: number; w: number; h: number },
	paper = 255,
	inkTone = 20,
) {
	const luma = new Uint8Array(w * h).fill(paper)
	for (let y = ink.y; y < ink.y + ink.h; y++) {
		for (let x = ink.x; x < ink.x + ink.w; x++) luma[y * w + x] = inkTone
	}
	return luma
}

describe('contentBox (Mihon border detection, capped per side)', () => {
	it('crops white margins around the art', () => {
		const box = contentBox(page(200, 300, { x: 20, y: 30, w: 160, h: 240 }), 200, 300)
		// one pixel of slack around the art
		expect(box).toEqual({ x: 19, y: 29, w: 162, h: 242 })
	})

	it('crops a black bleed margin too', () => {
		const luma = page(200, 300, { x: 0, y: 0, w: 200, h: 300 }, 255, 0) // all black
		// light art in the middle of a black page
		for (let y = 40; y < 260; y++) for (let x = 30; x < 170; x++) luma[y * 200 + x] = 230
		expect(contentBox(luma, 200, 300)).toEqual({ x: 29, y: 39, w: 142, h: 222 })
	})

	it("doesn't crop a side the art runs off", () => {
		// the art touches the left and top edges with light and dark mixed (a halftone,
		// not lined up with the every-other-pixel sampling)
		const luma = page(200, 300, { x: 0, y: 0, w: 150, h: 250 })
		for (let y = 0; y < 300; y += 4) luma[y * 200] = 255
		for (let x = 0; x < 200; x += 4) luma[x] = 255
		const box = contentBox(luma, 200, 300)
		expect(box?.x).toBe(0)
		expect(box?.y).toBe(0)
	})

	it('never takes more than 15% from a side', () => {
		// a tiny drawing on a big blank page: the cap keeps most of the page
		const box = contentBox(page(200, 200, { x: 90, y: 90, w: 20, h: 20 }), 200, 200)
		expect(box).toEqual({ x: 30, y: 30, w: 140, h: 140 })
	})

	it('leaves a page alone when there is nothing worth cropping', () => {
		// a margin of 1px is under 1% of the page
		expect(contentBox(page(200, 300, { x: 1, y: 1, w: 198, h: 298 }), 200, 300)).toBeNull()
		// a blank page
		expect(contentBox(new Uint8Array(200 * 300).fill(255), 200, 300)).toBeNull()
	})

	it('ignores specks under 0.25% of a line', () => {
		const luma = page(400, 400, { x: 40, y: 40, w: 320, h: 320 })
		// a dust speck in the top margin
		luma[10 * 400 + 200] = 0
		expect(contentBox(luma, 400, 400)?.y).toBe(39)
	})

	it('crops each half of a spread on its own', () => {
		// two pages side by side, each with its own margins
		const luma = page(400, 300, { x: 20, y: 30, w: 160, h: 240 })
		for (let y = 50; y < 250; y++) for (let x = 240; x < 360; x++) luma[y * 400 + x] = 20
		const crop = pageCrop(luma, { w: 400, h: 300, factor: 1 }, { w: 400, h: 300 }, true)
		expect(crop.left).toEqual({ x: 19, y: 29, w: 162, h: 242 })
		// its margins are 20% of the half: capped at 15% a side
		expect(crop.right).toEqual({ x: 230, y: 45, w: 140, h: 210 })
	})
})

describe('sampling and scaling', () => {
	it('shrinks a big page to about a megapixel', () => {
		const sample = sampleSize(2000, 3000)
		expect(sample.factor).toBe(3)
		expect(sample.w * sample.h).toBeLessThanOrEqual(1_000_000)
		expect(sampleSize(800, 1000).factor).toBe(1)
	})

	it('scales a box back to the page, clamped to it', () => {
		expect(scaleBox({ x: 10, y: 10, w: 100, h: 200 }, 3, { w: 330, h: 600 })).toEqual({
			x: 30,
			y: 30,
			w: 300,
			h: 570,
		})
	})

	it('turns RGBA into grey, transparent pixels as paper', () => {
		const rgba = new Uint8ClampedArray([255, 255, 255, 255, 0, 0, 0, 255, 0, 0, 0, 0])
		expect([...toLuma(rgba, 3)]).toEqual([255, 0, 255])
	})
})
