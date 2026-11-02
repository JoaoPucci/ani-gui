/**
 * A provider's subtitle label as the captions menu falls back to it
 * ({@link import('./caption-label')}): the SDH mark read out of it,
 * and the label cleaned of that mark and of the provider's
 * punctuation. Split from `caption-label.ts` so each file stays inside
 * the CRAP gate's per-file bar.
 */

const SDH = /(?<![\p{L}\p{N}])(?:sdh|hearing[\s-]*impaired)(?![\p{L}\p{N}])/giu;
const ROUND = /\(([^()[\]]*)\)/g;
const SQUARE = /\[([^()[\]]*)\]/g;
const EDGES = /^[\s\-–|,;]+|[\s\-–|,;]+$/g;

/** Whether `label` marks a track for the deaf and hard of hearing. */
export function isSdh(label: string): boolean {
	return label.search(SDH) !== -1;
}

/** A bracketed qualifier's parts, cleaned and rejoined in their
 *  brackets; a space when nothing is left of them. */
function qualifier(inner: string, open: string, close: string): string {
	const parts = inner
		.split(/[|,;]/)
		.map((part) => part.replace(EDGES, ''))
		.filter((part) => part !== '');
	return parts.length === 0 ? ' ' : `${open}${parts.join(', ')}${close}`;
}

function cleanOnce(label: string): string {
	return label
		.replace(SDH, ' ')
		.replace(ROUND, (_, inner: string) => qualifier(inner, '(', ')'))
		.replace(SQUARE, (_, inner: string) => qualifier(inner, '[', ']'))
		.replace(/([([])[\s\-–]+/g, '$1')
		.replace(/\s+/g, ' ')
		.replace(EDGES, '');
}

/** `label` without the SDH mark, the dash a provider opens a
 *  qualifier with, or a qualifier left empty. */
export function cleanLabel(label: string): string {
	let current = label;
	for (;;) {
		const next = cleanOnce(current);
		if (next === current) return next;
		current = next;
	}
}
