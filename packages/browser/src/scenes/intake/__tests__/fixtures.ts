import { IntakeKind, IntakeSource, IntakeState, LibraryType } from '@stump/graphql'

import type { IntakeItemData } from '../queries'

/** An inbox item that is ready to be added: a book of Umberto Eco in the Biblioteca library */
export const item = (overrides: Partial<IntakeItemData> = {}): IntakeItemData => ({
	createdAt: '2026-11-01T00:00:00Z',
	decision: {
		fileNames: ['El_nombre_de_la_rosa.epub'],
		folder: 'Umberto_Eco',
		metadata: {
			authors: ['Umberto Eco'],
			genres: ['Historical Fiction'],
			sources: [{ field: 'title', source: IntakeSource.File }],
			title: 'El nombre de la rosa',
		},
		volumes: [null],
	},
	destination: {
		dirExists: true,
		files: [
			{
				exists: false,
				originalName: 'rosa.epub',
				relativePath: 'Umberto_Eco/El_nombre_de_la_rosa.epub',
			},
		],
		relativeDir: 'Umberto_Eco',
	},
	files: [{ format: 'epub', originalName: 'rosa.epub', size: 1000 }],
	id: 'a',
	kind: IntakeKind.Book,
	libraryId: 'lib',
	libraryName: 'Biblioteca',
	libraryType: LibraryType.Book,
	mediaIds: [],
	name: 'rosa.epub',
	problems: [],
	proposal: {
		authorFolders: [],
		confidence: 0.9,
		duplicates: [],
		genreSuggestions: [
			{
				confidence: 0.8,
				kind: 'GENRE',
				libraryCount: 40,
				reasons: ['3 of 4 books by this author have it'],
				selected: true,
				value: 'Historical Fiction',
			},
			{
				confidence: 0.3,
				kind: 'FACET',
				libraryCount: 12,
				reasons: ['2 books with a similar description have it'],
				selected: false,
				value: 'Origin: Italian',
			},
		],
		lookedUpOnline: true,
		metadata: { authors: ['Umberto Eco'], genres: [], sources: [], subjects: [] },
		seriesFolders: [],
		similarBooks: [],
		warnings: [],
	},
	state: IntakeState.Ready,
	suggestedLibrary: null,
	...overrides,
})
