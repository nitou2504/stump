import { copyText } from '../copy'

const setSecure = (value: boolean) =>
	Object.defineProperty(window, 'isSecureContext', { value, configurable: true })
const setClipboard = (clipboard: unknown) =>
	Object.defineProperty(navigator, 'clipboard', { value: clipboard, configurable: true })

describe('copyText', () => {
	const execCommand = vi.fn()

	beforeEach(() => {
		execCommand.mockReset()
		Object.defineProperty(document, 'execCommand', { value: execCommand, configurable: true })
	})

	afterEach(() => {
		setSecure(true)
		setClipboard(undefined)
	})

	it('uses the Clipboard API where it exists', async () => {
		const writeText = vi.fn().mockResolvedValue(undefined)
		setSecure(true)
		setClipboard({ writeText })

		await expect(copyText('hello')).resolves.toBe(true)

		expect(writeText).toHaveBeenCalledWith('hello')
		expect(execCommand).not.toHaveBeenCalled()
	})

	it('selects the text and copies it the old way on a plain-http address, where there is no Clipboard API', async () => {
		setSecure(false)
		setClipboard(undefined)
		execCommand.mockReturnValue(true)
		let copiedValue: string | undefined
		execCommand.mockImplementation(() => {
			copiedValue = (document.activeElement as HTMLTextAreaElement | null)?.value
			return true
		})

		await expect(copyText('http://192.168.0.17/link')).resolves.toBe(true)

		expect(execCommand).toHaveBeenCalledWith('copy')
		expect(copiedValue).toBe('http://192.168.0.17/link')
		// the helper field is gone again
		expect(document.querySelectorAll('textarea')).toHaveLength(0)
	})

	it('falls back when the Clipboard API refuses', async () => {
		setSecure(true)
		setClipboard({ writeText: vi.fn().mockRejectedValue(new Error('denied')) })
		execCommand.mockReturnValue(true)

		await expect(copyText('x')).resolves.toBe(true)

		expect(execCommand).toHaveBeenCalledWith('copy')
	})

	it('says so when nothing could copy it', async () => {
		setSecure(false)
		setClipboard(undefined)
		execCommand.mockReturnValue(false)

		await expect(copyText('x')).resolves.toBe(false)
		expect(document.querySelectorAll('textarea')).toHaveLength(0)
	})

	it('says so when even the old way throws', async () => {
		setSecure(false)
		setClipboard(undefined)
		execCommand.mockImplementation(() => {
			throw new Error('not allowed')
		})

		await expect(copyText('x')).resolves.toBe(false)
		expect(document.querySelectorAll('textarea')).toHaveLength(0)
	})
})
