import {
	extractDocumentBody,
	extractNote,
	findTargetElement,
	isNoteElement,
	noteElements,
	parseBookPage,
} from '../noteContent'

const BASE = 'http://localhost:10801/api/v2/epub/book/resource/OEBPS/Text/notas.xhtml'

// ePubLibre: notes on their own page, out of the reading order, each with a way back
const EPUBLIBRE_NOTES = `<?xml version="1.0" encoding="utf-8"?>
<html xmlns="http://www.w3.org/1999/xhtml" xml:lang="es">
<head><title>Notas</title><link href="../Styles/style.css" rel="stylesheet" type="text/css"/></head>
<body>
  <h1>Notas</h1>
  <div class="nota">
    <p id="nt1"><sup>[1]</sup> Literalmente: «Señorita Corazones Solitarios». (<i>N. del t</i>.) <a href="epublibre01.xhtml#rf1">&lt;&lt;</a></p>
  </div>
  <div class="nota">
    <p id="nt2"><sup>[2]</sup> Otra nota, con <a href="https://example.com/x">un enlace</a>. <a href="epublibre03.xhtml#rf2">&lt;&lt;</a></p>
  </div>
</body>
</html>`

// EPUB 3: the note on the same page, in an aside
const EPUB3_PAGE = `<?xml version="1.0" encoding="utf-8"?>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
<body>
  <p>Text<a epub:type="noteref" role="doc-noteref" href="#n10_0" id="rn10_0">1</a> more text.</p>
  <aside epub:type="footnote" role="doc-footnote" id="n10_0">
    <p><a href="#rn10_0" role="doc-backlink">1.</a> The <em>first</em> note.<script>alert(1)</script></p>
  </aside>
  <p><a id="fn5"></a>5. An old-style note behind an empty anchor.</p>
  <dl><dt id="d1">7</dt><dd>A note in a definition list.</dd></dl>
  <p id="pic"><img src="../Images/fig.png" alt="Figure" onerror="alert(1)"/><img src="javascript:alert(1)"/></p>
</body>
</html>`

describe('parseBookPage', () => {
	it('reads XHTML pages, and falls back to HTML for broken ones', () => {
		expect(parseBookPage(EPUBLIBRE_NOTES).getElementById('nt1')).not.toBeNull()
		const broken = parseBookPage('<html><body><p id="x">unclosed<br></body></html>')
		expect(broken.getElementById('x')?.textContent).toContain('unclosed')
	})
})

describe('extractNote', () => {
	it('copies an ePubLibre note without its link back to the text', () => {
		const doc = parseBookPage(EPUBLIBRE_NOTES)
		const note = extractNote(doc, 'nt1', {
			baseHref: BASE,
			sourcePath: 'OEBPS/Text/epublibre01.xhtml',
			refId: 'rf1',
		})
		expect(note?.text).toBe('[1] Literalmente: «Señorita Corazones Solitarios». (N. del t.)')
		expect(note?.html).toContain('<sup>[1]</sup>')
		expect(note?.html).toContain('<i>N. del t</i>')
		expect(note?.html).not.toContain('&lt;&lt;')
		expect(note?.html).not.toContain('<a')
	})

	it('keeps the text of other links but not the links', () => {
		const doc = parseBookPage(EPUBLIBRE_NOTES)
		const note = extractNote(doc, 'nt2', { baseHref: BASE })
		expect(note?.text).toBe('[2] Otra nota, con un enlace.')
		expect(note?.html).not.toContain('href')
	})

	it('copies an EPUB 3 aside without its back-link or scripts', () => {
		const doc = parseBookPage(EPUB3_PAGE)
		const note = extractNote(doc, 'n10_0', { baseHref: BASE, refId: 'rn10_0' })
		expect(note?.text).toBe('The first note.')
		expect(note?.html).toContain('<em>first</em>')
		expect(note?.html).not.toContain('script')
		expect(note?.html).not.toContain('alert')
	})

	it('takes the paragraph around an empty anchor, and a term with its definition', () => {
		const doc = parseBookPage(EPUB3_PAGE)
		expect(extractNote(doc, 'fn5', { baseHref: BASE })?.text).toBe(
			'5. An old-style note behind an empty anchor.',
		)
		expect(extractNote(doc, 'd1', { baseHref: BASE })?.text).toBe('7 A note in a definition list.')
	})

	it('keeps images with a resolved address and nothing else', () => {
		const doc = parseBookPage(EPUB3_PAGE)
		const note = extractNote(doc, 'pic', { baseHref: BASE })
		expect(note?.html).toBe(
			'<div><img src="http://localhost:10801/api/v2/epub/book/resource/OEBPS/Images/fig.png" alt="Figure"></div>',
		)
	})

	it('has nothing to show for a missing id', () => {
		expect(extractNote(parseBookPage(EPUB3_PAGE), 'nope', { baseHref: BASE })).toBeNull()
	})
})

describe('extractDocumentBody', () => {
	it('copies a whole page for a link without an id', () => {
		const note = extractDocumentBody(parseBookPage(EPUBLIBRE_NOTES), { baseHref: BASE })
		expect(note?.text.startsWith('Notas [1] Literalmente')).toBe(true)
		expect(note?.html).toContain('<h1>Notas</h1>')
	})
})

describe('note elements', () => {
	it('finds old `name` anchors too', () => {
		const doc = parseBookPage(
			'<html><body><p><a name="old1"></a>Old note text here.</p></body></html>',
		)
		expect(findTargetElement(doc, 'old1')?.getAttribute('name')).toBe('old1')
	})

	it('knows a note by its EPUB 3 type or ARIA role, also on an ancestor', () => {
		const doc = parseBookPage(EPUB3_PAGE)
		expect(isNoteElement(doc.getElementById('n10_0'))).toBe(true)
		expect(isNoteElement(doc.querySelector('#n10_0 em'))).toBe(true)
		expect(isNoteElement(doc.getElementById('fn5'))).toBe(false)
	})

	it('uses a long inline target as is', () => {
		const doc = parseBookPage(
			'<html><body><p>Before <span id="s">a long enough inline note text</span> after</p></body></html>',
		)
		const [element] = noteElements(doc.getElementById('s')!)
		expect(element?.localName).toBe('span')
	})
})
