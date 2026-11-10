/**
 * A rehype plugin that takes out of a rendered description what runs: scripts, frames, embedded
 * objects, forms, `on*` handlers and `javascript:` addresses.
 *
 * Descriptions are Markdown with the HTML in it (EPUB's `dc:description`, an online source's blurb):
 * `rehype-raw` turns that HTML into elements, so a description from a book somebody else sent could
 * otherwise run a script as whoever reads it. The server already makes the description of a book from
 * somebody who needs approval plain text; this is the screen's own defence for every other source.
 */

/** The elements that are removed with what is inside them */
const BLOCKED_ELEMENTS = new Set([
	'script',
	'iframe',
	'frame',
	'frameset',
	'object',
	'embed',
	'applet',
	'portal',
	'base',
	'meta',
	'link',
	'form',
	'input',
	'button',
	'textarea',
	'select',
	'style',
])

type HastNode = {
	type: string
	tagName?: string
	properties?: Record<string, unknown>
	children?: HastNode[]
}

/** An address as a browser reads it: its controls and spaces dropped, in lower case */
const normalisedAddress = (value: string) =>
	// eslint-disable-next-line no-control-regex
	value.replace(/[\u0000-\u0020\u007f-\u009f]+/g, '').toLowerCase()

const RUNNING_ADDRESS = /^(javascript|vbscript|data:text\/html|data:application\/xhtml)/

const isActiveValue = (value: unknown): boolean => {
	if (Array.isArray(value)) return value.some(isActiveValue)
	return typeof value === 'string' && RUNNING_ADDRESS.test(normalisedAddress(value))
}

export const stripActiveContent = (node: HastNode): void => {
	if (node.children) {
		node.children = node.children.filter(
			(child) => !(child.type === 'element' && child.tagName && BLOCKED_ELEMENTS.has(child.tagName)),
		)
		node.children.forEach(stripActiveContent)
	}
	if (node.type === 'element' && node.properties) {
		for (const [name, value] of Object.entries(node.properties)) {
			if (/^on/i.test(name) || name.toLowerCase() === 'srcdoc' || isActiveValue(value)) {
				delete node.properties[name]
			}
		}
	}
}

/** `rehypePlugins={[rehypeRaw, rehypeStripActive]}`: after the raw HTML is elements */
export const rehypeStripActive = () => (tree: HastNode) => stripActiveContent(tree)
