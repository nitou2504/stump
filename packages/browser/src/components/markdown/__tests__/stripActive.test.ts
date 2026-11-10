import { describe, expect, it } from 'vitest'

import { stripActiveContent } from '../stripActive'

const element = (
	tagName: string,
	properties: Record<string, unknown> = {},
	children: unknown[] = [],
) => ({ type: 'element', tagName, properties, children })
const text = (value: string) => ({ type: 'text', value })

const run = (...children: any[]) => {
	const root = { type: 'root', children }
	stripActiveContent(root)
	return root.children
}

describe('stripActiveContent', () => {
	it('keeps ordinary markup as it is', () => {
		const tree = run(
			element('p', { className: ['a'] }, [
				text('hi '),
				element('a', { href: 'https://example.com' }),
			]),
			element('img', { src: 'cover.png', alt: 'x' }),
		)
		expect(tree).toHaveLength(2)
		expect(tree[0].children[1].properties.href).toBe('https://example.com')
	})

	it('removes scripts, frames, objects, forms and what is inside them', () => {
		const tree = run(
			element('p', {}, [text('keep')]),
			element('script', {}, [text('alert(1)')]),
			element('iframe', { src: 'x' }),
			element('object', {}, [element('p', {}, [text('inner')])]),
			element('form', { action: 'x' }, [element('input')]),
			element('style', {}, [text('body {}')]),
		)
		expect(tree).toHaveLength(1)
		expect(tree[0].tagName).toBe('p')
	})

	it('removes handlers and addresses that run, however they are written', () => {
		const tree = run(
			element('img', { src: 'a.png', onError: 'alert(1)', onload: 'x()' }),
			element('a', { href: 'javascript:alert(1)' }),
			element('a', { href: '  JaVa\tScRiPt:alert(1)' }),
			element('a', { href: 'vbscript:msgbox(1)' }),
			element('a', { href: 'data:text/html;base64,PHNjcmlwdD4=' }),
			element('iframe2', { srcDoc: '<script>' }),
		)
		expect(tree[0].properties).toEqual({ src: 'a.png' })
		for (const link of tree.slice(1, 5)) expect(link.properties.href).toBeUndefined()
		expect(tree[5].properties.srcDoc).toBeUndefined()
	})

	it('removes them from deep inside', () => {
		const tree = run(
			element('div', {}, [
				element('div', {}, [element('p', { onclick: 'x()' }, [element('script')])]),
			]),
		)
		const p = tree[0].children[0].children[0]
		expect(p.properties.onclick).toBeUndefined()
		expect(p.children).toHaveLength(0)
	})
})
