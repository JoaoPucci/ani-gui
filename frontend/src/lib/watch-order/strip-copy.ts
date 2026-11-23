/**
 * The strip's copy for the backend's readings (`$lib/api` `StripWire`)
 * and `./strip-wording`: Paraglide messages only, so the components
 * stay thin.
 */
import { m } from '$lib/paraglide/messages';
import { loopText, numeral } from './meta';
import type { FranchiseSummary, Seam, StripCaption, StripKind } from '$lib/api';
import type { ColumnKey, ColumnWord, EmptyColumn } from './strip-wording';

const WORDS: Record<ColumnWord, readonly [() => string, () => string]> = {
	prequel: [m.watchorder_strip_word_prequel, m.watchorder_strip_word_prequels],
	sequel: [m.watchorder_strip_word_sequel, m.watchorder_strip_word_sequels],
	parent: [m.watchorder_strip_word_parent, m.watchorder_strip_word_parents],
	released_before: [
		m.watchorder_strip_word_released_before,
		m.watchorder_strip_word_released_before
	],
	released_after: [m.watchorder_strip_word_released_after, m.watchorder_strip_word_released_after]
};

/** The key over a column a chain's seam stands in. */
export function seamKeyText(seam: Seam): string {
	return seam === 'from' ? m.watchorder_strip_seam_from() : m.watchorder_strip_seam_into();
}

export function columnKeyText(key: ColumnKey): string {
	return WORDS[key.word][key.plural ? 1 : 0]();
}

/**
 * The line under a key where a column holds several entries: a join or
 * a fork. A side story's several parents are neither, and get none, and
 * a franchise read in part (`complete` false) gets none: its count may
 * be short.
 */
export function columnCountText(
	kind: StripKind,
	side: 'before' | 'after',
	count: number,
	complete: boolean
): string | null {
	if (kind === 'side' || count < 2 || !complete) return null;
	return side === 'before'
		? m.watchorder_strip_join({ count })
		: m.watchorder_strip_ways({ count });
}

/** Why a column is empty: the story starts here, or nothing follows yet.
 *  The end of the story and a side story's end read the same. */
export function emptyColumnText(kind: EmptyColumn): string {
	if (kind === 'first') return m.watchorder_strip_first();
	return m.watchorder_strip_last();
}

/** The strip's caption ("05 of 09", "Side story", …), as its heading reads it. */
export function captionText(caption: StripCaption): string {
	switch (caption.kind) {
		case 'position':
			return m.watchorder_strip_position({
				position: numeral(caption.position),
				total: numeral(caption.total)
			});
		case 'numeral':
			return m.watchorder_strip_position_open({ position: numeral(caption.position) });
		case 'partial':
			return '';
		case 'side':
			return m.watchorder_strip_side_story();
		case 'film':
			return m.watchorder_strip_film();
		case 'cycle':
			return loopText(caption.count);
		case 'branch':
			return m.watchorder_strip_unnumbered_branch();
		case 'alone':
			return '';
	}
}

/**
 * The count the stack's last blank card carries: every title the
 * franchise page lists, or null for a franchise read in part, whose
 * count may be short.
 */
export function stackTotal(summary: FranchiseSummary, complete: boolean): number | null {
	return complete ? summary.total : null;
}

/**
 * What the franchise page holds, as the strip's link to it lists it
 * under "See the whole franchise": the main story, then films and
 * other versions, side stories and recaps, each only where there is one.
 * None for a franchise read in part, whose counts may be short.
 */
export function stackPartsText(summary: FranchiseSummary, complete: boolean): string[] {
	if (!complete) return [];
	const parts = [m.watchorder_strip_franchise_stack_main({ count: summary.main })];
	if (summary.films > 0)
		parts.push(m.watchorder_strip_franchise_stack_films({ count: summary.films }));
	if (summary.sides > 0)
		parts.push(m.watchorder_strip_franchise_stack_sides({ count: summary.sides }));
	if (summary.recaps > 0)
		parts.push(m.watchorder_strip_franchise_stack_recaps({ count: summary.recaps }));
	return parts;
}
