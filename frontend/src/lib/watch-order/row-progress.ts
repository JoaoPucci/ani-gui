/**
 * A franchise row's progress as the page draws it in the row's right
 * column: a caption, and a line under it only where the line measures
 * something.
 *
 * - Part-way through a title with a known total: "EP 6 of 14" with the
 *   line under it, filled to the share watched.
 * - Part-way through a title with no known total (still airing): "EP 5",
 *   with no line, since there is nothing to measure against.
 * - Finished: a check and "Watched", with no line. A full line on every
 *   finished row read as a marker, not a measurement.
 * - Not started: nothing.
 *
 * The caption is real text inside the row's link, so a screen reader
 * reads it with the title; the line is decoration.
 */
import type { FranchiseRow } from './franchise-sections';

export type RowProgress =
	| { readonly kind: 'none' }
	| { readonly kind: 'watched'; readonly caption: string }
	| { readonly kind: 'open'; readonly caption: string }
	| { readonly kind: 'partway'; readonly caption: string; readonly share: number };

/** One run of a caption: its digits are set in the numeral face. */
export interface CaptionPart {
	readonly text: string;
	readonly digits: boolean;
}

/** What `row`'s right column shows. */
export function rowProgress(row: Pick<FranchiseRow, 'entry' | 'progress' | 'share'>): RowProgress {
	void row;
	return { kind: 'none' };
}

/** `caption` in runs of digits and of everything else, in order. */
export function captionParts(caption: string): CaptionPart[] {
	return [{ text: caption, digits: false }];
}
