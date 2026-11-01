import { UserPermission } from '@stump/graphql'
import { Navigate } from 'react-router'

import { useAppContext } from '@/context'
import { usePaths } from '@/paths'

import IntakeScene from './IntakeScene'

/** The "Add books" page, for users with the Add books permission */
export default function IntakeRoute() {
	const { checkPermission } = useAppContext()
	const paths = usePaths()

	if (!checkPermission(UserPermission.AddBooks)) {
		return <Navigate to={paths.home()} replace />
	}

	return <IntakeScene />
}
