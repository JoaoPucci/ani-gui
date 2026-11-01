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

/** Declared ahead of its rules. */
export function emptyView(): SearchView {
	return { q: '', order: 'relevance', subtypes: [], anchor: null, restoredIds: new Set() };
}

/** Declared ahead of its rules. */
export function restoredView(saved: SearchSnapshot): SearchView {
	void saved;
	return emptyView();
}

/** Declared ahead of its rules. */
export function arrive(saved: SearchSnapshot | null, type: string, q: string): Arrival {
	void saved;
	void type;
	void q;
	return { kind: 'none' };
}
