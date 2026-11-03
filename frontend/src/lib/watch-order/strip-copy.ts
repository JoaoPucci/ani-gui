/**
 * The strip's copy for the readings in `./strip-view` and
 * `./strip-wording`: Paraglide messages only, so the components stay
 * thin.
 */
import { m } from '$lib/paraglide/messages';
import type { EmptyColumn } from './strip-view';
import type { ColumnKey, ColumnWord } from './strip-wording';

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

export function columnKeyText(key: ColumnKey): string {
	return WORDS[key.word][key.plural ? 1 : 0]();
}

/**
 * The line under a key where a column holds several entries: a join or
 * a fork. A side story's several parents are neither, and get none.
 */
export function columnCountText(
	kind: 'run' | 'side',
	side: 'before' | 'after',
	count: number
): string | null {
	if (kind === 'side' || count < 2) return null;
	return side === 'before'
		? m.watchorder_strip_join({ count })
		: m.watchorder_strip_ways({ count });
}

export function emptyColumnText(kind: EmptyColumn): string {
	if (kind === 'first') return m.watchorder_strip_first();
	if (kind === 'last') return m.watchorder_strip_last();
	return m.watchorder_strip_side_end();
}
