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
})
