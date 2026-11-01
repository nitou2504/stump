import { Button, cn, ProgressBar } from '@stump/components'
import { IntakeUploadMode } from '@stump/graphql'
import { CheckCircle2, CircleAlert, FolderUp, Loader2, Upload } from 'lucide-react'
import { useCallback, useRef, useState } from 'react'
import { useDropzone } from 'react-dropzone'

import type { UploadEntry } from './hooks'
import type { PendingFile } from './reviewState'

type Props = {
	onFiles: (files: PendingFile[], folderMode: IntakeUploadMode) => void
	entries: UploadEntry[]
	onClearFinished: () => void
	disabled?: boolean
}

/** react-dropzone (file-selector) gives dropped folder files a `path` like `./Yotsuba/v1.cbz` */
function relativePathOf(file: File): string | undefined {
	const withPaths = file as File & { path?: string; webkitRelativePath?: string }
	const raw = withPaths.webkitRelativePath || withPaths.path || ''
	const cleaned = raw.replace(/^\.?\/+/, '')
	return cleaned.includes('/') ? cleaned : undefined
}

const formatSize = (bytes: number) =>
	bytes >= 1024 * 1024 * 1024
		? `${(bytes / 1024 / 1024 / 1024).toFixed(1)} GB`
		: bytes >= 1024 * 1024
			? `${(bytes / 1024 / 1024).toFixed(1)} MB`
			: `${Math.max(1, Math.round(bytes / 1024))} KB`

/**
 * Drop books, folders of volumes or .zip packs here. Each upload goes to the inbox and is
 * analysed right away.
 */
export default function IntakeDropZone({ onFiles, entries, onClearFinished, disabled }: Props) {
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

	const finished = entries.filter((e) => e.status === 'done').length

	return (
		<section className="gap-3 flex flex-col" aria-label="Upload books">
			<div
				{...getRootProps()}
				className={cn(
					'gap-3 p-6 sm:p-8 flex flex-col items-center justify-center rounded-xl border-2 border-dashed text-center transition-colors',
					isDragActive ? 'border-primary bg-primary/5' : 'border-border bg-muted/20',
					disabled && 'opacity-60',
				)}
			>
				<input {...getInputProps()} />
				<Upload className="h-8 w-8 text-muted-foreground" aria-hidden />
				<div>
					<p className="font-medium">
						{isDragActive ? 'Drop to upload' : 'Drop books, folders or .zip files here'}
					</p>
					<p className="text-sm text-muted-foreground">
						EPUB, PDF, CBZ or CBR. A folder of volumes becomes one series.
					</p>
				</div>
				<div className="gap-2 flex flex-wrap justify-center">
					<Button size="sm" onClick={open} disabled={disabled}>
						<Upload /> Choose files
					</Button>
					<Button
						size="sm"
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
				<label className="gap-2 text-xs flex items-center text-muted-foreground">
					A folder is
					<select
						value={folderMode}
						onChange={(e) => setFolderMode(e.target.value as IntakeUploadMode)}
						className="h-8 px-2 text-base md:text-xs rounded-md border border-border bg-background text-foreground"
					>
						<option value={IntakeUploadMode.Auto}>one series (manga, light novels)</option>
						<option value={IntakeUploadMode.Books}>separate books</option>
					</select>
				</label>
			</div>

			{entries.length > 0 && (
				<div className="gap-1.5 flex flex-col">
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
					<ul className="max-h-60 divide-y divide-border overflow-y-auto rounded-lg border border-border">
						{entries.map((entry) => (
							<li key={entry.key} className="gap-3 px-3 py-2 flex items-center">
								{entry.status === 'done' ? (
									<CheckCircle2 className="h-4 w-4 shrink-0 text-success" aria-label="Uploaded" />
								) : entry.status === 'error' ? (
									<CircleAlert className="h-4 w-4 shrink-0 text-destructive" aria-label="Failed" />
								) : (
									<Loader2
										className={cn(
											'h-4 w-4 shrink-0 text-muted-foreground',
											entry.status === 'uploading' && 'animate-spin',
										)}
										aria-label={entry.status === 'queued' ? 'Waiting' : 'Uploading'}
									/>
								)}
								<div className="min-w-0 flex flex-1 flex-col">
									<span className="text-sm truncate" title={entry.label}>
										{entry.label}
										{entry.isFolder && (
											<span className="text-muted-foreground"> · {entry.files.length} files</span>
										)}
									</span>
									{entry.status === 'error' ? (
										<span className="text-xs text-destructive">{entry.error}</span>
									) : entry.status === 'uploading' ? (
										<ProgressBar value={entry.progress} size="sm" className="mt-1" />
									) : null}
								</div>
								<span className="text-xs shrink-0 text-muted-foreground">
									{entry.status === 'uploading'
										? `${entry.progress}%`
										: formatSize(entry.totalBytes)}
								</span>
							</li>
						))}
					</ul>
				</div>
			)}
		</section>
	)
}
