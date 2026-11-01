import { useSDK } from '@stump/client'
import { Link, Text } from '@stump/components'
import { useEffect, useState } from 'react'

type Props = {
	/**
	 * The ID of the media
	 */
	id: string
}

/**
 * A barebones PDF viewer that uses the native browser PDF viewer. This is done
 * by fetching the PDF as a blob and creating an object URL for it to be displayed
 * in an <object> tag.
 *
 * Eventually, a custom PDF viewer should be implemented as to support essential features
 * such as reading progress, bookmarks, and annotations.
 */
export default function NativePDFViewer({ id }: Props) {
	const { sdk } = useSDK()

	const [pdfObjectUrl, setPdfObjectUrl] = useState<string>()

	/**
	 * An effect that fetches the PDF and creates an object URL for it.
	 * When the component unmounts, the object URL is revoked.
	 */
	useEffect(() => {
		async function fetchPdf() {
			const response = await fetch(sdk.media.downloadURL(id), {
				credentials: 'include',
			})
			const blob = await response.blob()
			const arrayBuffer = await blob.arrayBuffer()
			setPdfObjectUrl(URL.createObjectURL(new Blob([arrayBuffer], { type: 'application/pdf' })))
		}

		if (!pdfObjectUrl) {
			fetchPdf()
		}

		return () => {
			if (pdfObjectUrl) {
				URL.revokeObjectURL(pdfObjectUrl)
			}
		}
	}, [sdk, id, pdfObjectUrl])

	// TODO: consider some sort of loading state here
	if (!pdfObjectUrl) {
		return null
	}

	return (
		<object data={pdfObjectUrl} type="application/pdf" width="100%" height="100%">
			{/* Shown by browsers without a built-in PDF viewer, e.g. most phones */}
			<div className="gap-2 p-6 flex h-full flex-col items-center justify-center text-center">
				<Text>This browser can&apos;t show the PDF here.</Text>
				<Link href={sdk.media.downloadURL(id)}>Download it instead</Link>
			</div>
		</object>
	)
}
