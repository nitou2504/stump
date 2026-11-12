import { SDKContext, StumpClientContext } from '@stump/client'
import { Api } from '@stump/sdk'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { renderHook, waitFor } from '@testing-library/react'
import { type PropsWithChildren } from 'react'

import { useCoreEvent } from '../useCoreEvent'

vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn(), info: vi.fn() } }))

function setup() {
	const sdk = new Api({ baseURL: 'http://localhost:10801', authMethod: 'session' })
	const connect = vi.spyOn(sdk, 'connect').mockResolvedValue({
		socket: { close: vi.fn() } as unknown as WebSocket,
		unsubscribe: vi.fn(),
	} as never)
	const client = new QueryClient()
	function Wrapper({ children }: PropsWithChildren) {
		return (
			<SDKContext.Provider value={{ sdk, setSDK: () => {} }}>
				<StumpClientContext.Provider value={{}}>
					<QueryClientProvider client={client}>{children}</QueryClientProvider>
				</StumpClientContext.Provider>
			</SDKContext.Provider>
		)
	}
	return { connect, Wrapper }
}

describe('the live events of the server', () => {
	it('are not asked for while nobody is signed in (the public edge refuses the socket)', async () => {
		const { connect, Wrapper } = setup()

		const { rerender } = renderHook(({ enabled }) => useCoreEvent({ enabled }), {
			initialProps: { enabled: false },
			wrapper: Wrapper,
		})
		// give an effect that was going to run its chance to
		await new Promise((resolve) => setTimeout(resolve, 50))
		expect(connect).not.toHaveBeenCalled()

		// the person signs in: now it connects, once
		rerender({ enabled: true })
		await waitFor(() => expect(connect).toHaveBeenCalledOnce())
		rerender({ enabled: true })
		await new Promise((resolve) => setTimeout(resolve, 50))
		expect(connect).toHaveBeenCalledOnce()
	})

	it('are asked for as before when nothing says otherwise', async () => {
		const { connect, Wrapper } = setup()

		renderHook(() => useCoreEvent({}), { wrapper: Wrapper })

		await waitFor(() => expect(connect).toHaveBeenCalledOnce())
	})
})
