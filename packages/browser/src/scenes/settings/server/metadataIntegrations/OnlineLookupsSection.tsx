import { useGraphQL, useGraphQLMutation } from '@stump/client'
import { Button, Heading, PasswordInput, Text } from '@stump/components'
import { extractErrorMessage, graphql } from '@stump/graphql'
import { useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { toast } from 'sonner'

const query = graphql(`
	query OnlineLookupSettings {
		onlineLookupSettings {
			googleBooksApiKeySet
		}
	}
`)

const mutation = graphql(`
	mutation UpdateOnlineLookupSettings($googleBooksApiKey: String) {
		updateOnlineLookupSettings(googleBooksApiKey: $googleBooksApiKey) {
			googleBooksApiKeySet
		}
	}
`)

/**
 * The key Add books and Suggest genres use to look books up on Google Books. Without one, Google
 * answers keyless requests from a quota every anonymous caller shares, which is often used up.
 * The key is stored encrypted and never shown again
 */
export default function OnlineLookupsSection() {
	const client = useQueryClient()
	const { data } = useGraphQL(query, ['onlineLookupSettings'])
	const isSet = data?.onlineLookupSettings.googleBooksApiKeySet ?? false
	const [key, setKey] = useState('')

	const { mutate, isPending } = useGraphQLMutation(mutation, {
		onError: (error) =>
			toast.error('Could not save the key', { description: extractErrorMessage(error) }),
		onSuccess: ({ updateOnlineLookupSettings: { googleBooksApiKeySet } }) => {
			toast.success(
				googleBooksApiKeySet ? 'Google Books API key saved' : 'Google Books API key removed',
			)
			setKey('')
			client.invalidateQueries({ queryKey: ['onlineLookupSettings'] })
		},
	})

	return (
		<section className="gap-4 flex flex-col" aria-label="Online lookups">
			<div>
				<Heading size="sm">Online lookups</Heading>
				<Text size="sm" variant="muted" className="mt-1 max-w-prose">
					A Google Books API key gives this server its own daily quota.
				</Text>
			</div>
			<form
				className="gap-3 sm:flex-row sm:items-end max-w-xl flex flex-col"
				onSubmit={(event) => {
					event.preventDefault()
					if (key.trim()) mutate({ googleBooksApiKey: key.trim() })
				}}
			>
				<div className="flex-1">
					<PasswordInput
						label="Google Books API key"
						placeholder={isSet ? 'Type a new key to replace it' : 'Optional'}
						value={key}
						onChange={(event) => setKey(event.target.value)}
						autoComplete="off"
						fullWidth
					/>
				</div>
				<div className="gap-2 flex">
					<Button type="submit" disabled={!key.trim() || isPending} isLoading={isPending}>
						Save
					</Button>
					{isSet && (
						<Button
							type="button"
							variant="outline"
							disabled={isPending}
							onClick={() => mutate({ googleBooksApiKey: null })}
						>
							Remove
						</Button>
					)}
				</div>
			</form>
			{isSet && (
				<Text size="xs" variant="muted">
					A key is set.
				</Text>
			)}
		</section>
	)
}
