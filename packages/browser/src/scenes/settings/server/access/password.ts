/**
 * Letters and digits that can't be mistaken for one another when the password is read out or
 * typed from a message: no `0`/`O`, `1`/`l`/`I`
 */
export const PASSWORD_ALPHABET = 'ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789'

/** The length of a password made here: 20 characters of 56 is about 116 bits */
export const GENERATED_PASSWORD_LENGTH = 20

/** The shortest and longest password the server takes (it checks again) */
export const MIN_PASSWORD_LENGTH = 8
export const MAX_PASSWORD_LENGTH = 128

type RandomBytes = (length: number) => Uint8Array

const browserRandomBytes: RandomBytes = (length) => {
	const bytes = new Uint8Array(length)
	crypto.getRandomValues(bytes)
	return bytes
}

/**
 * A random password from [`PASSWORD_ALPHABET`], from the browser's secure random numbers.
 *
 * A byte is used only when it falls in the largest multiple of the alphabet's size, so every
 * character is equally likely (a plain remainder would favour the first few). `randomBytes` is
 * there for a test to give it known bytes
 */
export function generatePassword(
	length: number = GENERATED_PASSWORD_LENGTH,
	randomBytes: RandomBytes = browserRandomBytes,
): string {
	const size = PASSWORD_ALPHABET.length
	const limit = 256 - (256 % size)
	let password = ''
	while (password.length < length) {
		for (const byte of randomBytes(length * 2)) {
			if (byte >= limit) continue
			password += PASSWORD_ALPHABET[byte % size]
			if (password.length === length) break
		}
	}
	return password
}
