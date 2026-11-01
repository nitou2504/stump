import type { PdfTextRun } from '@stump/sdk'
import { memo, useMemo } from 'react'

import { CHAR_ATTRIBUTE } from './selection'
import { canvasMeasureText, layoutRuns, TEXT_LAYER_FONT_FAMILY } from './textLayout'

type Props = {
	runs: PdfTextRun[]
	/** The page's width / height in points */
	aspect: number
	/** The page's CSS height; the runs' font sizes are fractions of it */
	pageHeight: number
}

/**
 * The page's text as transparent, selectable spans laid over the picture. Each span sits on
 * its run's box: the font size is the run's height and a horizontal stretch makes the text
 * as wide as the run, so selecting and copying behave like real text. A line break follows
 * runs that end a line, so copied text keeps them.
 */
function PdfTextLayer({ runs, aspect, pageHeight }: Props) {
	const layout = useMemo(() => layoutRuns(runs, aspect, canvasMeasureText()), [runs, aspect])

	return (
		<div className="pdf-text-layer" data-pdf-text-layer="">
			{layout.map((run, index) => (
				<TextRun key={`${run.firstChar}-${index}`} run={run} pageHeight={pageHeight} />
			))}
			<div className="pdf-text-end" aria-hidden />
		</div>
	)
}

type TextRunProps = {
	run: ReturnType<typeof layoutRuns>[number]
	pageHeight: number
}

function TextRun({ run, pageHeight }: TextRunProps) {
	const fontSize = Math.max(1, run.fontSize * pageHeight)
	return (
		<>
			<span
				{...{ [CHAR_ATTRIBUTE]: run.firstChar }}
				style={{
					left: `${run.left * 100}%`,
					top: `${run.top * 100}%`,
					fontSize: `${fontSize}px`,
					fontFamily: TEXT_LAYER_FONT_FAMILY,
					transform: run.scaleX !== 1 ? `scaleX(${run.scaleX})` : undefined,
				}}
			>
				{run.text}
			</span>
			{run.eol && <br role="presentation" />}
		</>
	)
}

export default memo(PdfTextLayer)
