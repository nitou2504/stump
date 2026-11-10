import { Button, cn, NativeSelect, ProgressBar } from '@stump/components'
import { IntakeUploadMode } from '@stump/graphql'
import { CheckCircle2, CircleAlert, Clock, FolderUp, Loader2, Upload } from 'lucide-react'
import { useCallback, useRef, useState } from 'react'
import { useDropzone } from 'react-dropzone'

import type { UploadEntry } from './hooks'
import { formatBytes, type PendingFile } from './reviewState'

type Props = {
	onFiles: (files: PendingFile[], folderMode: IntakeUploadMode) => void
	entries: UploadEntry[]
	onClearFinished: () => void
	disabled?: boolean
	/** The inbox already has books: the zone shrinks to a bar so they come first */
	compact?: boolean
	/** The libraries a book can be added to, and the one picked (a switch shows for two or more) */
	libraries?: Array<{ id: string; name: string }>
	libraryId?: string
	onLibraryChange?: (libraryId: string) => void
}

/** react-dropzone (file-selector) gives dropped folder files a `path` like `./Yotsuba/v1.cbz` */
function relativePathOf(file: File): string | undefined {
	const withPaths = file as File & { path?: string; webkitRelativePath?: string }
	const raw = withPaths.webkitRelativePath || withPaths.path || ''
	const cleaned = raw.replace(/^\.?\/+/, '')
	return cleaned.includes('/') ? cleaned : undefined
}

/**
 * Where books come in: drop books, folders of volumes or .zip packs, or choose them. A big,
 * friendly target while the inbox is empty and a slim bar once it isn't. Each upload goes to the
 * inbox and is analysed right away; its progress shows underneath.
 */
export default function IntakeDropZone({
	onFiles,
	entries,
	onClearFinished,
	disabled,
	compact,
	libraries = [],
	libraryId,
	onLibraryChange,
}: Props) {
	const [folderMode, setFolderMode] = useState<IntakeUploadMode>(IntakeUploadMode.Auto)
	const folderInput = useRef<HTMLInputElement>(null)

	const handle = useCallback(
		(files: File[]) =>
			onFiles(
				files.map((file) => ({ file, relativePath: relativePathOf(file) })),
				folderMode,
			),
		[onFiles, folderMode],
	)

	const { getRootProps, getInputProps, isDragActive, open } = useDropzone({
		disabled,
		multiple: true,
		noClick: true,
		onDrop: handle,
	})

	const slim = !!compact || entries.length > 0
	const finished = entries.filter((e) => e.status === 'done').length

	const buttons = (
		<div className="gap-2 flex flex-wrap justify-center">
			<Button size={slim ? 'sm' : 'lg'} onClick={open} disabled={disabled}>
				<Upload /> Choose files
			</Button>
			<Button
				size={slim ? 'sm' : 'lg'}
				variant="outline"
				onClick={() => folderInput.current?.click()}
				disabled={disabled}
			>
				<FolderUp /> Choose a folder
			</Button>
			<input
				ref={folderInput}
				type="file"
				multiple
				className="hidden"
				// eslint-disable-next-line @typescript-eslint/ban-ts-comment
				// @ts-ignore: non-standard but supported by every current browser
				webkitdirectory=""
				onChange={(e) => {
					handle(Array.from(e.target.files ?? []))
					e.target.value = ''
				}}
			/>
		</div>
	)

	return (
		<section className="gap-3 flex flex-col" aria-label="Upload books">
			<div
				{...getRootProps()}
				className={cn(
					'gap-4 flex flex-col rounded-2xl border-2 border-dashed transition-colors',
					slim ? 'p-3 sm:p-4' : 'p-6 sm:p-10',
					isDragActive ? 'border-primary bg-primary/5' : 'border-border bg-muted/20',
					disabled && 'opacity-60',
				)}
			>
				<input {...getInputProps()} />

				{slim ? (
					<div className="gap-3 flex flex-wrap items-center justify-between">
						<div className="gap-3 flex items-center">
							<span className="size-9 grid shrink-0 place-items-center rounded-full bg-primary/10 text-primary">
								<Upload className="h-4 w-4" aria-hidden />
							</span>
							<p className="text-sm font-medium">
								{isDragActive ? 'Drop to add' : 'Drop more books here'}
							</p>
						</div>
						{buttons}
					</div>
				) : (
					<div className="gap-4 flex flex-col items-center text-center">
						<span className="size-14 grid place-items-center rounded-full bg-primary/10 text-primary">
							<Upload className="h-6 w-6" aria-hidden />
						</span>
						<div className="gap-1 flex flex-col">
							<p className="text-lg font-semibold">
								{isDragActive ? 'Drop to add' : 'Drop books here'}
							</p>
							<p className="text-sm text-muted-foreground">
								or choose them from your device. EPUB, PDF, CBZ, CBR or a zip; folders work too.
							</p>
						</div>
						{buttons}
					</div>
				)}

				<div
					className={cn(
						'gap-x-4 gap-y-2 text-xs flex flex-wrap items-center text-muted-foreground',
						!slim && 'justify-center',
					)}
				>
					{libraries.length > 1 && (
						<label className="gap-2 flex items-center">
							<span>Add to</span>
							<NativeSelect
								size="sm"
								className="text-base md:text-sm w-auto"
								value={libraryId}
								disabled={disabled}
								options={libraries.map((library) => ({ label: library.name, value: library.id }))}
								onChange={(e) => onLibraryChange?.(e.target.value)}
							/>
						</label>
					)}
					<label className="gap-2 flex items-center">
						<span>Each folder is</span>
						<select
							value={folderMode}
							onChange={(e) => setFolderMode(e.target.value as IntakeUploadMode)}
							className="h-8 px-2 text-base md:text-xs rounded-md border border-border bg-background text-foreground"
						>
							<option value={IntakeUploadMode.Auto}>one series</option>
							<option value={IntakeUploadMode.Books}>separate books</option>
						</select>
					</label>
				</div>
			</div>

			{entries.length > 0 && (
				<div className="gap-2 flex flex-col">
					<div className="flex items-center justify-between">
						<span className="text-xs text-muted-foreground">
							Uploads · {finished} of {entries.length} done
						</span>
						{finished > 0 && (
							<Button variant="ghost" size="xs" onClick={onClearFinished}>
								Clear finished
							</Button>
						)}
					</div>
					<ul className="max-h-72 divide-y divide-border overflow-y-auto rounded-xl border border-border bg-card">
						{entries.map((entry) => (
							<li key={entry.key} className="gap-3 px-3 py-2.5 flex items-center">
								{entry.status === 'done' ? (
									<CheckCircle2 className="h-4 w-4 shrink-0 text-success" aria-label="Uploaded" />
								) : entry.status === 'error' ? (
									<CircleAlert className="h-4 w-4 shrink-0 text-destructive" aria-label="Failed" />
								) : entry.status === 'queued' ? (
									<Clock className="h-4 w-4 shrink-0 text-muted-foreground" aria-label="Waiting" />
								) : (
									<Loader2
										className="h-4 w-4 animate-spin shrink-0 text-primary"
										aria-label="Uploading"
									/>
								)}
								<div className="min-w-0 gap-1 flex flex-1 flex-col">
									<span className="text-sm truncate" title={entry.label}>
										{entry.label}
										{entry.isFolder && (
											<span className="text-muted-foreground"> · {entry.files.length} files</span>
										)}
									</span>
									{entry.status === 'error' ? (
										<span className="text-xs text-destructive">{entry.error}</span>
									) : entry.status === 'uploading' ? (
										<ProgressBar
											value={entry.progress}
											size="default"
											variant="primary"
											aria-label={`Uploading ${entry.label}`}
										/>
									) : entry.note ? (
										<span className="text-xs text-muted-foreground">{entry.note}</span>
									) : null}
								</div>
								<span className="text-xs shrink-0 text-muted-foreground tabular-nums">
									{entry.status === 'uploading'
										? `${entry.progress}%`
										: formatBytes(entry.totalBytes)}
								</span>
							</li>
						))}
					</ul>
				</div>
			)}
		</section>
	)
}
