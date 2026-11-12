import '@/__mocks__/resizeObserver'

import { render, screen, within } from '@testing-library/react'

import WarningsStrip from '../WarningsStrip'
import { setup } from './support'

vi.mock('@/hooks/useTranslate', () => import('./translations'))

const K = (key: string) => `settingsScene.server/access.${key}`

const warning = (id: string, kind: string, extra: Record<string, unknown> = {}) => ({
	id,
	kind,
	severity: 'WARN',
	count: 3,
	since: '2026-11-11T10:00:00Z',
	detail: null,
	linkId: null,
	person: { id: 'ana-id', username: 'ana', removed: false },
	...extra,
})

function renderStrip(warnings: ReturnType<typeof warning>[]) {
	const app = setup({
		answers: (query) => {
			if (query.includes('query PublicAccessWarnings')) return { accessWarnings: warnings }
			throw new Error(`Unexpected query ${query}`)
		},
	})
	return { app, ...render(<WarningsStrip />, { wrapper: app.Wrapper }) }
}

describe('Warnings', () => {
	it('shows nothing at all when there is nothing to look at', async () => {
		const { app } = renderStrip([])

		await vi.waitFor(() => expect(app.execute).toHaveBeenCalled())
		expect(screen.queryByTestId('access-warnings')).toBeNull()
	})

	it('says each thing in a sentence, with who or what it is about, and where on the page to deal with it', async () => {
		renderStrip([
			warning('AtDeviceLimit:ana', 'AT_DEVICE_LIMIT', { count: 3 }),
			warning('OverDeviceLimit:ana', 'OVER_DEVICE_LIMIT', { count: 4 }),
			warning('DeviceLimitHit:ana', 'DEVICE_LIMIT_HIT', { count: 2 }),
			warning('ManySessions:ana', 'MANY_SESSIONS', { count: 5 }),
			warning('SigninFailures:ana', 'SIGNIN_FAILURES', { count: 12 }),
			warning('SigninFailures:-', 'SIGNIN_FAILURES', { count: 40, person: null }),
			warning('NameBackoff:ana', 'NAME_BACKOFF'),
			warning('NameBackoff:-', 'NAME_BACKOFF', { person: null }),
			warning('NotEnabledTried:ana', 'NOT_ENABLED_TRIED'),
			warning('NotFriendClass:ana', 'NOT_FRIEND_CLASS', { detail: 'MANAGE_USERS, READ_USERS' }),
			warning('LinkNearBudget:l1', 'LINK_NEAR_BUDGET', { person: null, linkId: 'l1', detail: 'club' }),
			warning('LinkNoLibraries:l2', 'LINK_NO_LIBRARIES', { person: null, linkId: 'l2', detail: 'old' }),
		])

		const strip = await screen.findByTestId('access-warnings')
		const text = (key: string) => strip.textContent?.includes(key)
		expect(text(`${K('warnings.kinds.atDeviceLimit')} name=ana count=3`)).toBe(true)
		expect(text(`${K('warnings.kinds.overDeviceLimit')} name=ana count=4`)).toBe(true)
		expect(text(`${K('warnings.kinds.deviceLimitHit')} name=ana count=2`)).toBe(true)
		expect(text(`${K('warnings.kinds.manySessions')} name=ana count=5`)).toBe(true)
		expect(text(`${K('warnings.kinds.signinFailures')} name=ana count=12`)).toBe(true)
		// no person: the sentence about names that don't exist
		expect(text(`${K('warnings.kinds.signinFailuresUnknown')} name= count=40`)).toBe(true)
		expect(text(`${K('warnings.kinds.nameBackoffUnknown')}`)).toBe(true)
		expect(text(`${K('warnings.kinds.notFriendClass')} name=ana count=3 detail=MANAGE_USERS, READ_USERS`)).toBe(true)
		expect(text(`${K('warnings.kinds.linkNearBudget')} name= count=3 detail=club`)).toBe(true)
		expect(text(`${K('warnings.kinds.linkNoLibraries')} name= count=3 detail=old`)).toBe(true)

		// each says where to deal with it: the person, the links, the sign-ins
		const link = (id: string) =>
			within(screen.getByTestId(`access-warning-${id}`)).getByRole('link', { name: K('warnings.review') })
		expect(link('AtDeviceLimit:ana')).toHaveAttribute('href', '#access-people')
		expect(link('LinkNearBudget:l1')).toHaveAttribute('href', '#access-signups')
		expect(link('NameBackoff:ana')).toHaveAttribute('href', '#access-signins')
	})

	it('marks the alerts apart from the warnings, alerts first as the server sends them', async () => {
		renderStrip([
			warning('GlobalBudget:-', 'GLOBAL_BUDGET', { severity: 'ALERT', person: null, count: 1 }),
			warning('AtDeviceLimit:ana', 'AT_DEVICE_LIMIT'),
		])

		const alert = await screen.findByTestId('access-warning-GlobalBudget:-')
		expect(alert).toHaveAttribute('data-severity', 'ALERT')
		expect(screen.getByTestId('access-warning-AtDeviceLimit:ana')).toHaveAttribute(
			'data-severity',
			'WARN',
		)
		const ids = screen.getAllByTestId(/^access-warning-/).map((row) => row.dataset.testid)
		expect(ids[0]).toBe('access-warning-GlobalBudget:-')
	})
})
