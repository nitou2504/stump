import { useGraphQL, useGraphQLMutation } from '@stump/client'
import { Button, NativeSelect } from '@stump/components'
import { extractErrorMessage, UserPermission } from '@stump/graphql'
import { useQueryClient } from '@tanstack/react-query'
import { ChevronDown } from 'lucide-react'
import { useEffect, useId, useMemo, useRef, useState } from 'react'
import { useLocation, useSearchParams } from 'react-router'
import { toast } from 'sonner'

import { useAppContext } from '@/context'
import { useTranslate } from '@/hooks/useTranslate'
import {
	intakeLimitUsersQuery,
	updateUserIntakeLimitsMutation,
	type UserIntakeLimitsData,
	userIntakeLimitsQuery,
} from '@/scenes/intake/queries'

/** The words of the limits are all under one key in the locale files */
const key = (name: string) => `settingsScene.server/bookApprovals.limits.${name}`

/** The four limits an admin can give a person (see `UserIntakeLimitsInput`) */
const FIELDS = [
	{
		effective: (limits: UserIntakeLimitsData['effective']) =>
			Math.round(limits.maxFileBytes / 1024 / 1024),
		hint: 'mb',
		key: 'maxFileMb',
	},
	{
		effective: (limits: UserIntakeLimitsData['effective']) => limits.maxFilesPerRequest,
		hint: 'files',
		key: 'maxFilesPerRequest',
	},
	{
		effective: (limits: UserIntakeLimitsData['effective']) =>
			Math.round(limits.quotaBytes / 1024 / 1024),
		hint: 'mb',
		key: 'quotaMb',
	},
	{
		effective: (limits: UserIntakeLimitsData['effective']) => limits.maxItems,
		hint: 'books',
		key: 'maxItems',
	},
] as const

type FieldKey = (typeof FIELDS)[number]['key']
type Edits = Partial<Record<FieldKey, string>>

/** An empty box follows the server (`null`), else a whole number above zero; `undefined` is not one */
export function parseLimit(value: string): number | null | undefined {
	const trimmed = value.trim()
	if (!trimmed) return null
	if (!/^\d+$/.test(trimmed)) return undefined
	const number = Number(trimmed)
	return number > 0 && Number.isSafeInteger(number) ? number : undefined
}

/**
 * Settings → Book approvals → Upload limits: what each person who adds books may upload, for
 * those who manage users (the server answers to nobody else). The server's own limits are in
 * Stump.toml; here a person gets numbers of their own, and an empty box follows the server.
 */
export default function UserLimits() {
	const { checkPermission } = useAppContext()
	if (!checkPermission(UserPermission.ManageUsers)) return null
	return <UserLimitsSection />
}

function UserLimitsSection() {
	const { t, translate } = useTranslate()
	const { hash } = useLocation()
	const [params] = useSearchParams()
	const asked = params.get('person') ?? ''
	const { data } = useGraphQL(intakeLimitUsersQuery, ['intakeLimitUsers'])
	// the server owner files books themselves and is never held to these
	const users = useMemo(
		() =>
			(data?.users.nodes ?? []).filter(
				(user) => !user.isServerOwner && user.permissions.includes(UserPermission.AddBooks),
			),
		[data],
	)
	// closed until asked; it opens from the person's own sheet in Public access (`?person=`) or the anchor
	const [open, setOpen] = useState(!!asked || hash === '#limits')
	const [picked, setPicked] = useState(asked)
	const section = useRef<HTMLElement>(null)
	const userId = users.some((user) => user.id === picked) ? picked : ''

	useEffect(() => {
		if (asked || hash === '#limits') {
			setOpen(true)
			section.current?.scrollIntoView?.({ block: 'start' })
		}
	}, [asked, hash])

	return (
		<section
			id="limits"
			ref={section}
			aria-label={t(key('title'))}
			className="gap-3 pt-6 flex flex-col border-t border-border"
		>
			<button
				type="button"
				aria-expanded={open}
				aria-controls="limits-body"
				onClick={() => setOpen((next) => !next)}
				className="gap-2 flex w-full items-center justify-between text-left"
			>
				<span className="gap-1 flex flex-col">
					<span className="font-semibold">{t(key('title'))}</span>
					<span className="text-sm text-muted-foreground">{t(key('help'))}</span>
				</span>
				<ChevronDown aria-hidden className={open ? 'h-4 w-4 rotate-180' : 'h-4 w-4'} />
			</button>

			{open && (
				<div id="limits-body" className="gap-3 flex flex-col">
					{users.length === 0 ? (
						<p className="text-sm text-muted-foreground">
							{t(key('nobody'), {
								permission: t('userPermissions.ADD_BOOKS.label'),
								path: `${translate('shared.common.settings')} → ${t('settingsScene.server/users.title')}`,
							})}
						</p>
					) : (
						<>
							<label className="gap-2 text-sm flex items-center">
								{t(key('person'))}
								<NativeSelect
									size="sm"
									className="text-base md:text-sm w-auto"
									value={userId}
									emptyOption={{ label: t(key('choose')), value: '' }}
									options={users.map((user) => ({ label: user.username, value: user.id }))}
									onChange={(e) => setPicked(e.target.value)}
								/>
							</label>
							{userId && <LimitsForm key={userId} userId={userId} />}
						</>
					)}
				</div>
			)}
		</section>
	)
}

function LimitsForm({ userId }: { userId: string }) {
	const { t } = useTranslate()
	const id = useId()
	const client = useQueryClient()
	const queryKey = ['userIntakeLimits', userId]
	const { data } = useGraphQL(userIntakeLimitsQuery, queryKey, { userId })
	const limits = data?.userIntakeLimits
	// what was typed since the last save; anything else is what the person has now
	const [edits, setEdits] = useState<Edits>({})
	const save = useGraphQLMutation(updateUserIntakeLimitsMutation, {
		onError: (error) =>
			toast.error(t(key('saveError')), { description: extractErrorMessage(error) }),
		onSuccess: () => {
			setEdits({})
			toast.success(t(key('saved')))
			void client.invalidateQueries({ queryKey })
		},
	})

	if (!limits) return null

	const shown = (key: FieldKey) => edits[key] ?? (limits[key] == null ? '' : String(limits[key]))
	const parsed = Object.fromEntries(
		FIELDS.map(({ key }) => [key, parseLimit(shown(key))]),
	) as Record<FieldKey, number | null | undefined>
	const invalid = FIELDS.some(({ key }) => parsed[key] === undefined)
	const changed = FIELDS.some(({ key }) => key in edits && parsed[key] !== (limits[key] ?? null))
	const hasOwn = FIELDS.some(({ key }) => limits[key] != null)

	return (
		<form
			className="gap-3 flex flex-col"
			onSubmit={(e) => {
				e.preventDefault()
				if (invalid) return
				save.mutate({
					input: {
						maxFileMb: parsed.maxFileMb,
						maxFilesPerRequest: parsed.maxFilesPerRequest,
						maxItems: parsed.maxItems,
						quotaMb: parsed.quotaMb,
					},
					userId,
				})
			}}
		>
			<div className="gap-3 sm:grid-cols-2 grid grid-cols-1">
				{FIELDS.map(({ key: field, hint, effective }) => {
					const bad = parsed[field] === undefined
					return (
						<div key={field} className="gap-1 text-sm flex flex-col">
							<label htmlFor={`${id}-${field}`}>{t(key(field))}</label>
							<span className="gap-2 flex items-center">
								<input
									id={`${id}-${field}`}
									inputMode="numeric"
									value={shown(field)}
									placeholder={String(effective(limits.effective))}
									aria-invalid={bad}
									disabled={save.isPending}
									className="h-9 w-28 px-2 text-base md:text-sm rounded-md border border-border bg-input/30"
									onChange={(e) => setEdits((prev) => ({ ...prev, [field]: e.target.value }))}
								/>
								<span className="text-xs text-muted-foreground">{t(key(`hint.${hint}`))}</span>
							</span>
							{bad && <span className="text-xs text-destructive">{t(key('invalid'))}</span>}
						</div>
					)
				})}
			</div>

			<p className="text-xs text-muted-foreground">
				{t(key('now'), {
					file: FIELDS[0].effective(limits.effective),
					files: limits.effective.maxFilesPerRequest,
					quota: FIELDS[2].effective(limits.effective),
					items: limits.effective.maxItems,
				})}
			</p>

			<div className="gap-2 flex flex-wrap items-center">
				<Button type="submit" size="sm" disabled={!changed || invalid} isLoading={save.isPending}>
					{t(key('save'))}
				</Button>
				<Button
					type="button"
					size="sm"
					variant="ghost"
					disabled={!hasOwn || save.isPending}
					onClick={() => save.mutate({ input: {}, userId })}
				>
					{t(key('useServer'))}
				</Button>
			</div>
		</form>
	)
}
