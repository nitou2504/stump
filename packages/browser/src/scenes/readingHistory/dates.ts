import { format, parse } from 'date-fns'

/**
 * A day the server sends (`2026-03-05`: the logical day of a session, with no time and no zone)
 * as that same day on the calendar of whoever is looking. It is parsed, not given to
 * `new Date`, which reads it as midnight UTC and shows the day before west of Greenwich.
 */
export const parseDay = (value: string) => parse(value, 'yyyy-MM-dd', new Date())

/** `Mar 5, 2026`, in the language of the app */
export const formatDay = (value: string) => format(parseDay(value), 'PP')

/** The month a day belongs to, `2026-03`: what the finished books are grouped by */
export const monthOf = (value: string) => value.slice(0, 7)

/** `March 2026`, in the language of the app, for a month as `monthOf` gives it */
export const formatMonth = (month: string) => format(parseDay(`${month}-01`), 'LLLL yyyy')
