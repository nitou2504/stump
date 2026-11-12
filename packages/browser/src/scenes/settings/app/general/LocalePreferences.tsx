import {
	Alert,
	AlertDescription,
	AlertTitle,
	Button,
	ButtonOrLink,
	dismissAlert,
} from '@stump/components'
import { Languages } from 'lucide-react'
import { useState } from 'react'

import { useAppContext } from '@/context'
import { useTranslate } from '@/hooks/useTranslate'

import LocaleSelector from './LocaleSelector'

/** Upstream's call for translators: it is the owner's to act on, so friends are not shown it */
function ContributeToTranslations() {
	const { t } = useTranslate()

	// just to trigger a rerender since alert doesn't subscribe to localStorage changes
	// its fine
	const [didDismiss, setDidDismiss] = useState(false)

	return (
		<Alert
			variant="info"
			className="relative"
			showX={false}
			dismissible
			id="contribute-to-translations"
			key={`contribute-to-translations-${didDismiss}`}
		>
			<Languages />
			<AlertTitle>{t(getKey('contribute'))}</AlertTitle>
			<AlertDescription className="flex flex-row items-start justify-between">
				{t(getKey('contributeIfAble'))}
			</AlertDescription>

			<div className="md:right-10 gap-x-2 md:absolute ml-4 mt-2 flex flex-row self-center">
				<Button
					variant="ghost"
					className="hover:bg-black/15"
					onClick={() => {
						dismissAlert('contribute-to-translations')
						setDidDismiss(true)
					}}
				>
					{t(getKey('noThanks'))}
				</Button>

				<ButtonOrLink
					href="https://hosted.weblate.org/engage/stump/"
					target="_blank"
					rel="noopener noreferrer"
					variant="outline"
				>
					{t(getKey('contribute'))}
				</ButtonOrLink>
			</div>
		</Alert>
	)
}

export default function LocalePreferences() {
	const { isServerOwner } = useAppContext()

	return (
		<div className="gap-y-4 flex flex-col">
			{isServerOwner && <ContributeToTranslations />}

			<LocaleSelector />
		</div>
	)
}

const getKey = (key: string) => `settingsScene.app/account.sections.language.${key}`
