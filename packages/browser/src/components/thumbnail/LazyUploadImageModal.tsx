import { ComponentProps, lazy, Suspense, useState } from 'react'

// The upload dialog brings react-dropzone along; it is only fetched once a user opens it
const UploadImageModal = lazy(() => import('./UploadImageModal'))

type Props = ComponentProps<typeof UploadImageModal>

/** {@link UploadImageModal}, downloaded the first time it opens and kept mounted afterwards */
export default function LazyUploadImageModal(props: Props) {
	const [wasOpened, setWasOpened] = useState(props.isOpen)
	if (props.isOpen && !wasOpened) {
		setWasOpened(true)
	}

	if (!wasOpened) return null

	return (
		<Suspense fallback={null}>
			<UploadImageModal {...props} />
		</Suspense>
	)
}
