import { useGraphQL, useGraphQLMutation } from '@stump/client'
import { Button, NativeSelect } from '@stump/components'
import { extractErrorMessage, UserPermission } from '@stump/graphql'
import { useQueryClient } from '@tanstack/react-query'
import { useId, useMemo, useState } from 'react'
import { toast } from 'sonner'

import { useAppContext } from '@/context'
import {
	intakeLimitUsersQuery,
	updateUserIntakeLimitsMutation,
	type UserIntakeLimitsData,
	userIntakeLimitsQuery,
} from '@/scenes/intake/queries'

/** The four limits an admin can give a person (see `UserIntakeLimitsInput`) */
const FIELDS = [
	{
		effective: (limits: UserIntakeLimitsData['effective']) =>
			Math.round(limits.maxFileBytes / 1024 / 1024),
		hint: 'MB',
		key: 'maxFileMb',
		label: 'Biggest file',
	},
	{
		effective: (limits: UserIntakeLimitsData['effective']) => limits.maxFilesPerRequest,
		hint: 'files',
		key: 'maxFilesPerRequest',
		label: 'Files in one upload',
	},
	{
		effective: (limits: UserIntakeLimitsData['effective']) =>
			Math.round(limits.quotaBytes / 1024 / 1024),
		hint: 'MB',
		key: 'quotaMb',
		label: 'Waiting in their inbox',
	},
	{
		effective: (limits: UserIntakeLimitsData['effective']) => limits.maxItems,
		hint: 'books',
		key: 'maxItems',
		label: 'Books open at once',
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
	const { data } = useGraphQL(intakeLimitUsersQuery, ['intakeLimitUsers'])
	// the server owner files books themselves and is never held to these
	const users = useMemo(
		() =>
			(data?.users.nodes ?? []).filter(
				(user) => !user.isServerOwner && user.permissions.includes(UserPermission.AddBooks),
			),
		[data],
	)
	const [picked, setPicked] = useState('')
	const userId = users.some((user) => user.id === picked) ? picked : users[0]?.id

	return (
		<section aria-label="Upload limits" className="gap-3 pt-6 flex flex-col border-t border-border">
			<div className="gap-1 flex flex-col">
				<h2 className="font-semibold">Upload limits</h2>
				<p className="text-sm text-muted-foreground">
					What each person who adds books may upload. An empty box follows the server’s limit.
				</p>
			</div>

			{!userId ? (
				<p className="text-sm text-muted-foreground">
					Nobody else can add books yet: give somebody <em>Add books</em> in Settings → Users.
				</p>
			) : (
				<>
					<label className="gap-2 text-sm flex items-center">
						Person
						<NativeSelect
							size="sm"
							className="text-base md:text-sm w-auto"
							value={userId}
							options={users.map((user) => ({ label: user.username, value: user.id }))}
							onChange={(e) => setPicked(e.target.value)}
						/>
					</label>
					<LimitsForm key={userId} userId={userId} />
				</>
			)}
		</section>
	)
}

function LimitsForm({ userId }: { userId: string }) {
	const id = useId()
	const client = useQueryClient()
	const queryKey = ['userIntakeLimits', userId]
	const { data } = useGraphQL(userIntakeLimitsQuery, queryKey, { userId })
	const limits = data?.userIntakeLimits
	// what was typed since the last save; anything else is what the person has now
	const [edits, setEdits] = useState<Edits>({})
	const save = useGraphQLMutation(updateUserIntakeLimitsMutation, {
		onError: (error) =>
			toast.error('Could not save the limits', { description: extractErrorMessage(error) }),
		onSuccess: () => {
			setEdits({})
			toast.success('Limits saved')
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
				{FIELDS.map(({ key, label, hint, effective }) => {
					const bad = parsed[key] === undefined
					return (
						<div key={key} className="gap-1 text-sm flex flex-col">
							<label htmlFor={`${id}-${key}`}>{label}</label>
							<span className="gap-2 flex items-center">
								<input
									id={`${id}-${key}`}
									inputMode="numeric"
									value={shown(key)}
									placeholder={String(effective(limits.effective))}
									aria-invalid={bad}
									disabled={save.isPending}
									className="h-9 w-28 px-2 text-base md:text-sm rounded-md border border-border bg-input/30"
									onChange={(e) => setEdits((prev) => ({ ...prev, [key]: e.target.value }))}
								/>
								<span className="text-xs text-muted-foreground">{hint}</span>
							</span>
							{bad && (
								<span className="text-xs text-destructive">
									A whole number above zero, or empty
								</span>
							)}
						</div>
					)
				})}
			</div>

			<p className="text-xs text-muted-foreground">
				Now: files up to {FIELDS[0].effective(limits.effective)} MB,{' '}
				{limits.effective.maxFilesPerRequest} files in one upload,{' '}
				{FIELDS[2].effective(limits.effective)} MB waiting and {limits.effective.maxItems} books
				open at once.
			</p>

			<div className="gap-2 flex flex-wrap items-center">
				<Button type="submit" size="sm" disabled={!changed || invalid} isLoading={save.isPending}>
					Save
				</Button>
				<Button
					type="button"
					size="sm"
					variant="ghost"
					disabled={!hasOwn || save.isPending}
					onClick={() => save.mutate({ input: {}, userId })}
				>
					Use the server’s limits
				</Button>
			</div>
		</form>
	)
}
