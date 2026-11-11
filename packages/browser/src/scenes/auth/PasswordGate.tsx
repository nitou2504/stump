import { lazy, Suspense } from 'react'

import { AppLayout } from '@/AppLayout'
import { useUserStore } from '@/stores'

const MustChangePasswordScene = lazy(() => import('./MustChangePasswordScene.tsx'))

/**
 * The layout of every signed-in page, except while the server owner's password is the one a
 * person has to replace: then they are shown only the screen that does it. The app's own
 * layout isn't mounted at all, since everything it asks for would be refused until the
 * password is chosen
 */
export default function PasswordGate() {
	const mustChangePassword = useUserStore((state) => !!state.user?.mustChangePassword)

	if (mustChangePassword) {
		return (
			<Suspense fallback={null}>
				<MustChangePasswordScene />
			</Suspense>
		)
	}

	return <AppLayout />
}
