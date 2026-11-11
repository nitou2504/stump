/**
 * Puts `text` on the clipboard and says whether it worked.
 *
 * The Clipboard API exists only on secure addresses (https, localhost): the owner often opens
 * Stump on a plain `http://192.168.…` LAN address, where it doesn't. There the text is selected in
 * a hidden field and copied with the old `execCommand`, which every browser still has.
 */
export async function copyText(text: string): Promise<boolean> {
	if (typeof navigator !== 'undefined' && navigator.clipboard && window.isSecureContext) {
		try {
			await navigator.clipboard.writeText(text)
			return true
		} catch {
			// refused (a permission, a lost focus): try the old way
		}
	}
	return copyWithSelection(text)
}

function copyWithSelection(text: string): boolean {
	const field = document.createElement('textarea')
	field.value = text
	field.setAttribute('readonly', '')
	// out of sight, but not `display: none` (a hidden field can't be selected)
	field.style.position = 'fixed'
	field.style.top = '0'
	field.style.left = '-9999px'
	field.style.opacity = '0'
	document.body.appendChild(field)
	try {
		field.focus()
		field.select()
		field.setSelectionRange(0, text.length)
		return document.execCommand('copy')
	} catch {
		return false
	} finally {
		document.body.removeChild(field)
	}
}
