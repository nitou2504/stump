import { Button, cn, Input } from '@stump/components'
import { useEffect, useRef, useState } from 'react'

import { useTranslate } from '@/hooks/useTranslate'

type Props = {
	/** The field's name, read out and shown as its placeholder's lead, e.g. "Go to page" */
	label: string
	/** What can be typed, e.g. `1 – 315` or `1 – 2310, or 45%` */
	placeholder: string
	/** What was typed, as the place to go (a page, a location), or `null` when it isn't one */
	resolve: (input: string) => number | null
	onGo: (place: number) => void
	/** Shown under the field when what was typed isn't a place */
	invalidMessage: string
	/** Focus the field as it shows (opened with `g` or from the position in the top bar) */
	autoFocus?: boolean
	/** Tells the field to clear itself, e.g. each time its panel opens */
	resetKey?: unknown
	className?: string
}

/**
 * "Go to": a page (or a location) typed in, the same field in every reader: at the top of
 * the contents (EPUB, PDF) or the pages (comics), and in the PDF's own go-to dialog. Enter
 * goes; a place that doesn't exist says so under the field.
 */
export default function GoToField({
	label,
	placeholder,
	resolve,
	onGo,
	invalidMessage,
	autoFocus = false,
	resetKey,
	className,
}: Props) {
	const { translate } = useTranslate()
	const [value, setValue] = useState('')
	const [invalid, setInvalid] = useState(false)
	const inputRef = useRef<HTMLInputElement>(null)

	useEffect(() => {
		setValue('')
		setInvalid(false)
	}, [resetKey])

	useEffect(() => {
		if (!autoFocus) return undefined
		// after the dialog has placed its own focus
		const frame = window.requestAnimationFrame(() => inputRef.current?.focus())
		return () => window.cancelAnimationFrame(frame)
	}, [autoFocus, resetKey])

	const submit = () => {
		const place = resolve(value)
		if (place == null) {
			setInvalid(true)
			return
		}
		onGo(place)
	}

	return (
		<form
			className={cn('gap-2 flex items-start', className)}
			onSubmit={(event) => {
				event.preventDefault()
				submit()
			}}
			data-testid="reader-go-to"
		>
			<div className="min-w-0 flex-1">
				<Input
					ref={inputRef}
					fullWidth
					inputMode="text"
					enterKeyHint="go"
					aria-label={label}
					placeholder={`${label}: ${placeholder}`}
					value={value}
					onChange={(event) => {
						setValue(event.target.value)
						setInvalid(false)
					}}
					errorMessage={invalid ? invalidMessage : undefined}
				/>
			</div>
			<Button type="submit" variant="secondary">
				{translate('shared.pdfReader.go')}
			</Button>
		</form>
	)
}

/**
 * Where a typed "go to" lands in a book of `total` places (pages or locations), from 1: a
 * number (`123`), or a share of the book (`45%`, `45 %`, `45.5%`). `null` for anything else
 * or for a number outside the book.
 */
export function resolvePlaceInput(input: string, total: number): number | null {
	const text = input.trim().replace(',', '.')
	if (!text || total < 1) return null
	const percent = /^(\d+(?:\.\d+)?)\s*%$/.exec(text)
	if (percent) {
		const share = Number(percent[1]) / 100
		if (!Number.isFinite(share) || share < 0 || share > 1) return null
		return Math.min(total, Math.max(1, Math.round(share * (total - 1)) + 1))
	}
	if (!/^\d+$/.test(text)) return null
	const place = Number(text)
	return place >= 1 && place <= total ? place : null
}
