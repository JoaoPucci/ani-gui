/**
 * The search page's result counter: the number, and the word after it.
 *
 * While Kitsu has more pages the number gains a "+", because it counts
 * what is shown so far rather than the whole catalogue. A "+" count
 * means more than the number, so its word is the open plural
 * (`search_count_word_open`) rather than the form that number takes.
 */

import { m } from '$lib/paraglide/messages';

export type ResultCount = { num: string; word: string };

/**
 * @param count hits shown so far
 * @param moreToCome whether Kitsu has pages the page has not loaded
 */
export function resultCount(count: number, moreToCome: boolean): ResultCount {
	if (moreToCome && count > 0) {
		return { num: m.search_count_more({ count }), word: m.search_count_word_open() };
	}
	return { num: String(count), word: m.search_count_word({ count }) };
}
