import { Suspense } from 'react'
import { Helmet } from 'react-helmet'

import { SceneContainer } from '@/components/container'
import { useTranslate } from '@/hooks/useTranslate'

import ActivitySection from './ActivitySection'
import DeviceSection from './DeviceSection'
import InviteSection from './InviteSection'
import { accessKey } from './keys'
import PeopleSection from './PeopleSection'
import SignInSection from './SignInSection'
import StatusStrip from './StatusStrip'

/**
 * *Public access*: how people come in from the internet. The owner makes invite links, sees the
 * devices that hold a pass and can cut them off, lets locked accounts back in and sets
 * passwords, all without a terminal. It is the owner's alone: the route is only there for the
 * server owner, and the server refuses everything on it to anyone else and to any request that
 * came through the public address
 */
export default function AccessScene() {
	const { t } = useTranslate()

	return (
		<SceneContainer variant="settings">
			<Helmet>
				<title>Stump | {t(accessKey('helmet'))}</title>
			</Helmet>

			<div className="gap-y-10 flex flex-col">
				<Suspense fallback={null}>
					<StatusStrip />
				</Suspense>
				<InviteSection />
				<DeviceSection />
				<SignInSection />
				<PeopleSection />
				<ActivitySection />
			</div>
		</SceneContainer>
	)
}
