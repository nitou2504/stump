/**
 * Styles of the PDF reader's pages. Plain CSS (injected once by the reader) because the
 * text layer needs selectors Tailwind can't express (`::selection`, sibling states).
 */
export const PDF_READER_CSS = `
.pdf-page {
	position: absolute;
	overflow: hidden;
	box-shadow: 0 1px 3px rgba(0, 0, 0, 0.25);
	contain: layout paint;
}
.pdf-page-image {
	position: absolute;
	inset: 0;
	width: 100%;
	height: 100%;
	user-select: none;
	-webkit-user-select: none;
	-webkit-touch-callout: none;
	pointer-events: none;
}
.pdf-page-tint {
	position: absolute;
	inset: 0;
	mix-blend-mode: multiply;
	pointer-events: none;
}
.pdf-page-error {
	position: absolute;
	inset: 0;
	display: flex;
	align-items: center;
	justify-content: center;
	padding: 1rem;
	font-size: 0.875rem;
	color: #6b6f76;
}
.pdf-annotation-layer {
	position: absolute;
	inset: 0;
	z-index: 1;
	pointer-events: none;
}
.pdf-annotation {
	position: absolute;
	border-radius: 1px;
}
.pdf-annotation--selected {
	outline: 2px solid rgba(37, 99, 235, 0.85);
	outline-offset: 1px;
}
.pdf-search-flash {
	position: absolute;
	border-radius: 2px;
	background: rgba(249, 115, 22, 0.35);
	outline: 2px solid rgba(249, 115, 22, 0.9);
	animation: pdf-flash 2.4s ease-out forwards;
}
@keyframes pdf-flash {
	0%, 70% { opacity: 1; }
	100% { opacity: 0; }
}
.pdf-text-layer {
	position: absolute;
	inset: 0;
	z-index: 2;
	line-height: 1;
	text-align: initial;
	-webkit-text-size-adjust: none;
	text-size-adjust: none;
	forced-color-adjust: none;
	caret-color: transparent;
}
.pdf-text-layer span,
.pdf-text-layer br {
	position: absolute;
	color: transparent;
	white-space: pre;
	cursor: text;
	transform-origin: 0 0;
}
.pdf-text-layer span::selection {
	background: rgba(37, 99, 235, 0.3);
	color: transparent;
}
.pdf-text-layer br::selection {
	background: transparent;
}
.pdf-text-end {
	position: absolute;
	inset: 100% 0 0;
	z-index: -1;
	cursor: default;
	user-select: none;
	-webkit-user-select: none;
}
.pdf-text-layer.selecting .pdf-text-end {
	top: 0;
}
.pdf-note-layer {
	position: absolute;
	inset: 0;
	z-index: 3;
	pointer-events: none;
}
.pdf-note-glyph {
	position: absolute;
	display: flex;
	width: 16px;
	height: 16px;
	align-items: center;
	justify-content: center;
	pointer-events: auto;
	opacity: 0.85;
}
.pdf-note-glyph:hover {
	opacity: 1;
}
@media (pointer: coarse) {
	.pdf-note-glyph {
		width: 28px;
		height: 28px;
		margin: -6px 0 0 -6px;
	}
}
`
