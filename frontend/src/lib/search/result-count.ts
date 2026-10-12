/**
 * The search page's result counter: the number, and the word after it.
 *
 * While Kitsu has more pages the number gains a "+", because it counts
 * what is shown so far rather than the whole catalogue.
 */

import { m } from '$lib/paraglide/messages';

export type ResultCount = { num: string; word: string };

/**
 * @param count hits shown so far
 * @param moreToCome whether Kitsu has pages the page has not loaded
 */
export function resultCount(count: number, moreToCome: boolean): ResultCount {
	const num = moreToCome && count > 0 ? m.search_count_more({ count }) : String(count);
	return { num, word: m.search_count_word({ count }) };
}
