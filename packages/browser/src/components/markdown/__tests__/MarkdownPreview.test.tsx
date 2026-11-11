import { render } from '@testing-library/react'

import MarkdownPreview, { normalizeDescription } from '../MarkdownPreview'

// An EPUB description with a pretty-printed (indented) second paragraph
const INDENTED_HTML =
	'<p>Divinas para unos, diabólicas para otros.</p>\n\n    <p>Ofreciendo al lector <i>crack</i>, opio y un largo etcétera.</p>\n'

describe('normalizeDescription', () => {
	it('drops line indentation from HTML descriptions', () => {
		expect(normalizeDescription(INDENTED_HTML)).not.toMatch(/^[ \t]+<p>/m)
	})

	it('keeps Markdown untouched (indented code blocks are meaningful there)', () => {
		const markdown = 'Some text\n\n    code block\n'
		expect(normalizeDescription(markdown)).toBe(markdown)
	})
})

describe('MarkdownPreview', () => {
	it('renders indented HTML paragraphs instead of showing their tags', () => {
		const { container } = render(<MarkdownPreview>{INDENTED_HTML}</MarkdownPreview>)
		expect(container.textContent).not.toContain('<p>')
		expect(container.textContent).toContain('Ofreciendo al lector crack')
		expect(container.querySelector('i')?.textContent).toBe('crack')
		expect(container.querySelector('pre, code')).toBeNull()
	})

	// What a friend's book can carry in its description: raw HTML that this component turns into
	// elements. The real pipeline is rendered (Markdown, raw HTML, the sanitizer), and the DOM it
	// makes is what is looked at, not a tree written by hand.
	describe('a description with scripts in it', () => {
		const HOSTILE = [
			'<p>keep this</p>',
			'<script>window.__ran = true</script>',
			'<img src="x.png" onerror="window.__ran = true" alt="an image">',
			'<a href="javascript:window.__ran=true">a script link</a>',
			'<a href="  JaVa&#x09;Script:window.__ran=true">a hidden link</a>',
			'<a href="data:text/html;base64,PHNjcmlwdD4=">a data link</a>',
			'<iframe src="https://evil.example/" srcdoc="<script>1</script>"></iframe>',
			'<object data="x.swf"><embed src="x.swf"></object>',
			'<form action="https://evil.example/"><input name="a"><button>send</button></form>',
			'<svg onload="window.__ran = true"><script>1</script></svg>',
			'<p style="x" onclick="window.__ran = true">clickable</p>',
		].join('\n')

		it('leaves no script, frame, form or handler in the page', () => {
			const { container } = render(<MarkdownPreview>{HOSTILE}</MarkdownPreview>)
			for (const tag of ['script', 'iframe', 'object', 'embed', 'form', 'input', 'button']) {
				expect(container.querySelector(tag), tag).toBeNull()
			}
			expect(container.querySelector('svg script')).toBeNull()
			// no element has an attribute that runs
			for (const element of Array.from(container.querySelectorAll('*'))) {
				for (const attribute of Array.from(element.attributes)) {
					expect(attribute.name.toLowerCase().startsWith('on'), attribute.name).toBe(false)
					expect(attribute.name.toLowerCase(), attribute.name).not.toBe('srcdoc')
					expect(attribute.value.replace(/\s/g, '').toLowerCase()).not.toMatch(
						/^(javascript|vbscript|data:text\/html)/,
					)
				}
			}
			expect((window as unknown as { __ran?: boolean }).__ran).toBeUndefined()
		})

		it('keeps the text, and the links that are links', () => {
			const { container } = render(
				<MarkdownPreview>
					{HOSTILE + '\n\n<p><a href="https://example.com/ok">a fine link</a></p>'}
				</MarkdownPreview>,
			)
			expect(container.textContent).toContain('keep this')
			expect(container.textContent).toContain('a script link')
			expect(container.querySelector('a[href="https://example.com/ok"]')?.textContent).toBe(
				'a fine link',
			)
		})
	})
})

