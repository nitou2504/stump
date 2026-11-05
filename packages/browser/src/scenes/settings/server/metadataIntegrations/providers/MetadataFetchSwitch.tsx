import { useGraphQL, useGraphQLMutation } from '@stump/client'
import { WideSwitch } from '@stump/components'
import { extractErrorMessage, graphql } from '@stump/graphql'
import { useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

const query = graphql(`
	query MetadataFetchSettings {
		metadataFetchSettings {
			fetchAfterScan
		}
	}
`)

const mutation = graphql(`
	mutation UpdateMetadataFetchSettings($fetchAfterScan: Boolean!) {
		updateMetadataFetchSettings(fetchAfterScan: $fetchAfterScan) {
			fetchAfterScan
		}
	}
`)

/**
 * Whether a scan asks the providers about the books it adds (and the scheduled retries run).
 * Off keeps their daily quota for people's lookups; fetches someone starts still run
 */
export function MetadataFetchSwitch() {
	const client = useQueryClient()
	const { data } = useGraphQL(query, ['metadataFetchSettings'])
	const isOn = data?.metadataFetchSettings.fetchAfterScan ?? true

	const { mutate, isPending } = useGraphQLMutation(mutation, {
		onError: (error) =>
			toast.error('Could not save the setting', { description: extractErrorMessage(error) }),
		onSuccess: () => client.invalidateQueries({ queryKey: ['metadataFetchSettings'] }),
	})

	return (
		<WideSwitch
			label="Fetch metadata for new books after a scan"
			description="Off saves the providers' daily quota for lookups."
			checked={isOn}
			disabled={!data || isPending}
			onCheckedChange={(fetchAfterScan) => mutate({ fetchAfterScan })}
		/>
	)
}
