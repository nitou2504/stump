import { renderHook, waitFor } from '@testing-library/react'

import { setup } from '@/scenes/settings/server/access/__tests__/support'

import { useInternetPerson } from '../useInternetPerson'

const PEOPLE = {
	accessPeople: [
		{ publicSignin: true, person: { id: 'lucia-id', username: 'lucia' } },
		{ publicSignin: false, person: { id: 'dee-id', username: 'dee' } },
	],
}

describe('whether the person being edited may sign in from the internet', () => {
	it('says the name of somebody who may', async () => {
		const app = setup({ answers: () => PEOPLE })

		const { result } = renderHook(() => useInternetPerson('lucia-id', true), {
			wrapper: app.Wrapper,
		})

		await waitFor(() => expect(result.current).toBe('lucia'))
	})

	it('says nothing for somebody who may not, or for an account that is new', async () => {
		const app = setup({ answers: () => PEOPLE })

		const { result } = renderHook(
			() => [useInternetPerson('dee-id', true), useInternetPerson(undefined, true)],
			{ wrapper: app.Wrapper },
		)

		await waitFor(() => expect(app.execute).toHaveBeenCalled())
		expect(result.current).toEqual([undefined, undefined])
	})

	it('does not even ask when it is not the owner who is looking', async () => {
		const app = setup({ answers: () => PEOPLE })

		const { result } = renderHook(() => useInternetPerson('lucia-id', false), {
			wrapper: app.Wrapper,
		})

		await new Promise((resolve) => setTimeout(resolve, 50))
		expect(result.current).toBeUndefined()
		expect(app.execute).not.toHaveBeenCalled()
	})
})
