import { Heading } from '@stump/components'

/** The logo and the name above every sign-in screen (login, join) */
export default function AuthLogo() {
	return (
		<div className="gap-4 px-2 flex shrink-0 items-center justify-center">
			<img src="/assets/stump-logo--irregular-256.webp" width="80" height="80" alt="" />
			<Heading variant="gradient" size="3xl" className="font-bold">
				Stump
			</Heading>
		</div>
	)
}
