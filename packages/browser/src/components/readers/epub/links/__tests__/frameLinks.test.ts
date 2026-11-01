import {
	attachFrameLinkHandling,
	type FrameLink,
	LONG_PRESS_MS,
	readLinkFacts,
} from '../frameLinks'

const PAGE = `
<p id="text">Some text with a note<sup><a id="rf1" href="notas.xhtml#nt1">[1]</a></sup> in it.</p>
<p><a id="toc" href="chapter02.xhtml"><span id="toc-inner">Chapter 2</span></a></p>
<p><a id="noteref" epub:type="noteref" role="doc-noteref" href="#n1">1</a></p>
<p><a id="js" href="javascript:void(0)">nothing</a></p>`

type PointerInit = { x?: number; y?: number; time?: number; type?: string; button?: number }

const frames: HTMLIFrameElement[] = []

function pointer(
	name: string,
	{ x = 10, y = 10, time = 0, type = 'mouse', button = 0 }: PointerInit,
) {
	const event = new MouseEvent(name, {
		bubbles: true,
		cancelable: true,
		clientX: x,
		clientY: y,
		button,
	})
	Object.defineProperty(event, 'pointerType', { value: type })
	Object.defineProperty(event, 'isPrimary', { value: true })
	Object.defineProperty(event, 'timeStamp', { value: time })
	return event
}

function setup() {
	// A frame's document, like the book's pages (and with a selection, unlike a bare document)
	const frame = document.createElement('iframe')
	document.body.appendChild(frame)
	frames.push(frame)
	const doc = frame.contentDocument!
	doc.body.innerHTML = PAGE
	const activated: FrameLink[] = []
	// Stands in for Readium's own listeners: bubbling, on the document
	const readium = vi.fn()
	doc.addEventListener('pointerup', readium)
	doc.addEventListener('click', readium)
	const detach = attachFrameLinkHandling(doc, (link) => activated.push(link))
	return { doc, activated, readium, detach }
}

function tap(target: Element, down: PointerInit = {}, up: PointerInit = down) {
	target.dispatchEvent(pointer('pointerdown', down))
	const event = pointer('pointerup', up)
	target.dispatchEvent(event)
	return event
}

describe('attachFrameLinkHandling', () => {
	afterEach(() => {
		for (const frame of frames.splice(0)) frame.remove()
	})

	it('takes a tap on a link away from Readium and reports the link', () => {
		const { doc, activated, readium } = setup()
		const event = tap(doc.getElementById('rf1')!, { type: 'touch' })

		expect(activated).toHaveLength(1)
		expect(activated[0]).toMatchObject({
			id: 'rf1',
			facts: { text: '[1]', superscript: true, epubTypes: [], roles: [] },
			document: doc,
		})
		expect(activated[0]?.href).toMatch(/notas\.xhtml#nt1$/)
		expect(event.defaultPrevented).toBe(true)
		expect(readium).not.toHaveBeenCalled()
	})

	it('reports a link tapped through an element inside it', () => {
		const { doc, activated } = setup()
		tap(doc.getElementById('toc-inner')!)
		expect(activated[0]?.facts.text).toBe('Chapter 2')
	})

	it('reads EPUB 3 note reference markup', () => {
		const { doc } = setup()
		expect(readLinkFacts(doc.getElementById('noteref')!)).toMatchObject({
			epubTypes: ['noteref'],
			roles: ['doc-noteref'],
		})
	})

	it('leaves a tap outside links to Readium (page turns, the controls)', () => {
		const { doc, activated, readium } = setup()
		tap(doc.getElementById('text')!)
		expect(activated).toHaveLength(0)
		expect(readium).toHaveBeenCalledTimes(1)
	})

	it('leaves swipes and drags that start on a link to Readium', () => {
		const { doc, activated, readium } = setup()
		tap(doc.getElementById('rf1')!, { x: 10 }, { x: 60 })
		expect(activated).toHaveLength(0)
		expect(readium).toHaveBeenCalledTimes(1)
	})

	it('leaves a text selection to Readium, which shows the annotation toolbar', () => {
		const { doc, activated, readium } = setup()
		const range = doc.createRange()
		range.selectNodeContents(doc.getElementById('text')!)
		doc.getSelection()!.addRange(range)

		tap(doc.getElementById('rf1')!)
		expect(activated).toHaveLength(0)
		expect(readium).toHaveBeenCalledTimes(1)
	})

	it('swallows a long press on a link without following it', () => {
		const { doc, activated, readium } = setup()
		tap(
			doc.getElementById('rf1')!,
			{ type: 'touch', time: 0 },
			{ type: 'touch', time: LONG_PRESS_MS + 100 },
		)
		expect(activated).toHaveLength(0)
		expect(readium).not.toHaveBeenCalled()
	})

	it('does not follow a link on a right click', () => {
		const { doc, activated } = setup()
		tap(doc.getElementById('rf1')!, { button: 2 })
		expect(activated).toHaveLength(0)
	})

	it('ignores javascript: links', () => {
		const { doc, activated, readium } = setup()
		tap(doc.getElementById('js')!)
		expect(activated).toHaveLength(0)
		expect(readium).toHaveBeenCalledTimes(1)
	})

	it('never lets the page itself navigate, and follows keyboard activations', () => {
		const { doc, activated } = setup()
		const mouseClick = new MouseEvent('click', { bubbles: true, cancelable: true, detail: 1 })
		doc.getElementById('rf1')!.dispatchEvent(mouseClick)
		expect(mouseClick.defaultPrevented).toBe(true)
		expect(activated).toHaveLength(0)

		const keyboardClick = new MouseEvent('click', { bubbles: true, cancelable: true, detail: 0 })
		doc.getElementById('toc')!.dispatchEvent(keyboardClick)
		expect(keyboardClick.defaultPrevented).toBe(true)
		expect(activated[0]?.facts.text).toBe('Chapter 2')
	})

	it('stops handling links once detached', () => {
		const { doc, activated, readium, detach } = setup()
		detach()
		tap(doc.getElementById('rf1')!)
		expect(activated).toHaveLength(0)
		expect(readium).toHaveBeenCalledTimes(1)
	})
})
