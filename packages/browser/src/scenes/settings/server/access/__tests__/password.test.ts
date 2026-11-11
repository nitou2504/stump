import {
	GENERATED_PASSWORD_LENGTH,
	generatePassword,
	MAX_PASSWORD_LENGTH,
	MIN_PASSWORD_LENGTH,
	PASSWORD_ALPHABET,
} from '../password'

describe('generatePassword', () => {
	it('is 20 characters of the alphabet, which has none that can be mistaken for another', () => {
		const password = generatePassword()

		expect(password).toHaveLength(GENERATED_PASSWORD_LENGTH)
		expect([...password].every((char) => PASSWORD_ALPHABET.includes(char))).toBe(true)
		for (const confusing of '0O1lI') {
			expect(PASSWORD_ALPHABET).not.toContain(confusing)
		}
	})

	it('is a length the server takes', () => {
		expect(GENERATED_PASSWORD_LENGTH).toBeGreaterThanOrEqual(MIN_PASSWORD_LENGTH)
		expect(GENERATED_PASSWORD_LENGTH).toBeLessThanOrEqual(MAX_PASSWORD_LENGTH)
	})

	it('is not the same twice', () => {
		const passwords = new Set(Array.from({ length: 20 }, () => generatePassword()))
		expect(passwords.size).toBe(20)
	})

	it('uses a byte only when it falls in the largest multiple of the alphabet, so no character is favoured', () => {
		const size = PASSWORD_ALPHABET.length
		const limit = 256 - (256 % size)
		// the bytes at and above the limit would make the first characters likelier: they are skipped
		expect(limit).toBeLessThan(256)

		const bytes = [0, 1, limit - 1, limit, 255, size, 7]
		const password = generatePassword(5, () => Uint8Array.from(bytes))

		expect(password).toBe(
			[
				PASSWORD_ALPHABET[0],
				PASSWORD_ALPHABET[1],
				PASSWORD_ALPHABET[(limit - 1) % size],
				PASSWORD_ALPHABET[0], // `size` itself
				PASSWORD_ALPHABET[7],
			].join(''),
		)
	})

	it('asks for more bytes when too many were skipped', () => {
		const size = PASSWORD_ALPHABET.length
		const limit = 256 - (256 % size)
		let calls = 0
		const password = generatePassword(3, () => {
			calls += 1
			// the first batch is all above the limit, the second is usable
			return calls === 1 ? new Uint8Array(6).fill(limit) : Uint8Array.from([1, 2, 3, 4, 5, 6])
		})

		expect(calls).toBe(2)
		expect(password).toBe([1, 2, 3].map((byte) => PASSWORD_ALPHABET[byte]).join(''))
	})

	it("draws from the browser's secure random numbers", () => {
		const spy = vi.spyOn(crypto, 'getRandomValues')
		generatePassword()
		expect(spy).toHaveBeenCalled()
		spy.mockRestore()
	})
})
