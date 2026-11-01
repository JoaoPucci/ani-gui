/**
 * What the search page does when a navigation lands on it, and the
 * state it shows as a result: the search page's restoration
 * transitions, kept out of the component (AGENTS.md §2). The component
 * supplies the runtime inputs (the snapshot SvelteKit handed back, the
 * navigation type, the address's query) and applies the outcome.
 */

import type { SearchOrder } from '$lib/api';
import type { ScrollAnchor, SearchSnapshot } from './snapshot';

/** The page state a search shows, outside its results. */
export interface SearchView {
	/** The query whose results are on screen; empty for none. */
	q: string;
	order: SearchOrder;
	subtypes: string[];
	/** The card to put back at the top once the results are up. */
	anchor: ScrollAnchor | null;
	/** Cards that do not rise in again. */
	restoredIds: Set<string>;
}

/** What a landing navigation asks of the page. */
export type Arrival =
	| { kind: 'none' }
	| { kind: 'follow'; q: string }
	| { kind: 'restore'; view: SearchView; saved: SearchSnapshot; reload: boolean };

/** The page with nothing asked: no search, Kitsu's ranking, no type
 *  filter. */
export function emptyView(): SearchView {
	return { q: '', order: 'relevance', subtypes: [], anchor: null, restoredIds: new Set() };
}

/** The view a saved search puts back. The cards it saved do not rise
 *  in again, whether they come back as they were or from the search
 *  run again. */
export function restoredView(saved: SearchSnapshot): SearchView {
	return {
		q: saved.q,
		order: saved.order,
		subtypes: [...saved.subtypes],
		anchor: saved.anchor,
		restoredIds: new Set(saved.results?.hits.map((h) => h.id))
	};
}

/**
 * What a navigation landing on the search page asks of it, given the
 * snapshot SvelteKit restored for the entry (if any), the navigation's
 * type, and the query the address names. A back or forward press, or
 * a reload, onto a saved search the address names restores it; a
 * press that cannot restore follows the address. Any other navigation
 * was already followed as the address changed.
 */
export function arrive(saved: SearchSnapshot | null, type: string, q: string): Arrival {
	if (type !== 'popstate' && type !== 'enter') return { kind: 'none' };
	if (saved && q && saved.q === q) {
		return { kind: 'restore', view: restoredView(saved), saved, reload: type === 'enter' };
	}
	return saved || type === 'popstate' ? { kind: 'follow', q } : { kind: 'none' };
}
