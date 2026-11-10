import { Helmet } from 'react-helmet'

import { SceneContainer } from '@/components/container'
import { useTranslate } from '@/hooks/useTranslate'
import { MY_READING_AVAILABLE } from '@/scenes/readingHistory/availability'
import { ReadingPrivacyCard } from '@/scenes/readingHistory/ReadingPrivacy'

import LocalePreferences from './LocalePreferences'
import ProfileForm from './ProfileForm'

export default function GeneralSettingsScene() {
	const { t } = useTranslate()

	return (
		<SceneContainer variant="settings">
			<Helmet>
				<title>Stump | {t('settingsScene.app/account.helmet')}</title>
			</Helmet>

			<ProfileForm />
			{MY_READING_AVAILABLE && <ReadingPrivacyCard />}
			<LocalePreferences />
		</SceneContainer>
	)
}
