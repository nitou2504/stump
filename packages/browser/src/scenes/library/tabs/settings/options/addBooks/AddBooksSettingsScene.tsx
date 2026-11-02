import { useGraphQL, useGraphQLMutation } from '@stump/client'
import { Button, Input, WideSwitch } from '@stump/components'
import { extractErrorMessage, graphql } from '@stump/graphql'
import { useQueryClient } from '@tanstack/react-query'
import { useEffect, useState } from 'react'
import { toast } from 'sonner'

import { SettingsSection } from '@/components/sharedLayout'
import { useLibraryContext } from '@/scenes/library/context'

const query = graphql(`
	query LibraryAddBooksSettings($libraryId: ID!) {
		libraryIntakeSettings(libraryId: $libraryId) {
			libraryId
			mangaDir
			lightNovelDir
			csvPath
			onlineLookup
		}
	}
`)

const mutation = graphql(`
	mutation UpdateLibraryAddBooksSettings($libraryId: ID!, $input: LibraryIntakeSettingsInput!) {
		updateLibraryIntakeSettings(libraryId: $libraryId, input: $input) {
			libraryId
		}
	}
`)

type Draft = { mangaDir: string; lightNovelDir: string; csvPath: string; onlineLookup: boolean }

/** Where "Add books" files books in this library, and whether it looks titles up online */
export default function AddBooksSettingsScene() {
	const {
		library: { id: libraryId },
	} = useLibraryContext()
	const client = useQueryClient()
	const queryKey = ['libraryIntakeSettings', libraryId]
	const { data, error } = useGraphQL(query, queryKey, { libraryId })
	const settings = data?.libraryIntakeSettings

	const [draft, setDraft] = useState<Draft>()
	useEffect(() => {
		if (settings) {
			setDraft({
				csvPath: settings.csvPath ?? '',
				lightNovelDir: settings.lightNovelDir,
				mangaDir: settings.mangaDir,
				onlineLookup: settings.onlineLookup,
			})
		}
	}, [settings])

	const { mutate, isPending } = useGraphQLMutation(mutation, {
		onError: (e) => toast.error('Could not save', { description: extractErrorMessage(e) }),
		onSuccess: () => {
			toast.success('Saved')
			client.invalidateQueries({ queryKey })
		},
	})

	if (error) {
		return (
			<p className="text-sm text-muted-foreground">
				These settings need the Add books permission: {extractErrorMessage(error)}
			</p>
		)
	}
	if (!draft || !settings) return null

	const changed =
		draft.mangaDir !== settings.mangaDir ||
		draft.lightNovelDir !== settings.lightNovelDir ||
		draft.csvPath !== (settings.csvPath ?? '') ||
		draft.onlineLookup !== settings.onlineLookup

	const save = () =>
		mutate({
			input: {
				csvPath: draft.csvPath.trim() || null,
				lightNovelDir: draft.lightNovelDir.trim(),
				mangaDir: draft.mangaDir.trim(),
				onlineLookup: draft.onlineLookup,
			},
			libraryId,
		})

	return (
		<div className="gap-10 flex flex-col">
			<SettingsSection
				title="Series folders"
				description="Manga and light novels are filed by series, one folder per series inside these folders (relative to the library root). Other books go to their author's folder."
			>
				<div className="gap-4 sm:grid-cols-2 grid">
					<Input
						label="Manga folder"
						value={draft.mangaDir}
						onChange={(e) => setDraft({ ...draft, mangaDir: e.target.value })}
						fullWidth
					/>
					<Input
						label="Light novel folder"
						value={draft.lightNovelDir}
						onChange={(e) => setDraft({ ...draft, lightNovelDir: e.target.value })}
						fullWidth
					/>
				</div>
			</SettingsSection>

			<SettingsSection
				title="Added books list"
				description="Every added book gets a row (catalogo.csv columns plus who added it and when) so the curator can pick it up."
			>
				<Input
					label="CSV file"
					placeholder="Default: stump_intake.csv next to catalogo.csv, or in the library root"
					value={draft.csvPath}
					onChange={(e) => setDraft({ ...draft, csvPath: e.target.value })}
					fullWidth
				/>
			</SettingsSection>

			<SettingsSection title="Online lookups">
				<WideSwitch
					label="Look books up on Open Library and Google Books"
					description="Fills in missing authors, series, year and subjects while analysing an upload"
					checked={draft.onlineLookup}
					onCheckedChange={(onlineLookup) => setDraft({ ...draft, onlineLookup })}
				/>
			</SettingsSection>

			<div>
				<Button onClick={save} disabled={!changed || isPending} isLoading={isPending}>
					Save
				</Button>
			</div>
		</div>
	)
}
