/**
 * The topbar's live search: what the dropdown shows as the viewer
 * types, and when a search goes out, is dropped or is cancelled.
 *
 * A query starts a search once the typing settles (`debounceMs`) and
 * only when it is long enough (`minChars`); a shorter one clears the
 * dropdown. Each search runs in a visit of its own
 * (`$lib/navigation/visit`): the next keystroke, closing the dropdown
 * to navigate, or the layout going cancels it, since it would answer
 * nobody and it holds an API connection until Kitsu answers. A search
 * cancelled so shows nothing, neither its hits nor a failure. An
 * answer for a query the viewer has typed past is dropped.
 *
 * The view records the query its hits answer, so closing the dropdown
 * drops hits of a query the viewer typed past: focusing the field
 * again never shows them under the new query.
 *
 * The layout is the adapter: it hands over the query as typed, the
 * search and filter calls, and writes each view it is handed.
 */

import type { KitsuAnimeRef } from '$lib/api';
import { visitSeries } from '$lib/navigation/visit';

/** What the dropdown shows of the live search. */
export interface LiveSearchView {
	/** The hits of the query searched last; null with nothing searched. */
	results: KitsuAnimeRef[] | null;
	/** A search is out. */
	busy: boolean;
	/** The last search failed. */
	error: boolean;
}

export interface LiveSearchDeps {
	/** Kitsu's search for `q`, cancelled with `signal`. */
	search: (q: string, signal: AbortSignal) => Promise<KitsuAnimeRef[]>;
	/** Drops hits the cache knows are not streamable, cancelled with `signal`. */
	filter: (hits: KitsuAnimeRef[], signal: AbortSignal) => Promise<KitsuAnimeRef[]>;
	/** The query in the field now, trimmed. */
	currentQuery: () => string;
	/** Hands the layout each new view. */
	changed: (view: LiveSearchView) => void;
	debounceMs: number;
	minChars: number;
	maxHits: number;
	/** The timer; `setTimeout` unless a test gives one. */
	schedule?: (run: () => void, ms: number) => unknown;
	cancel?: (handle: unknown) => void;
}

export interface LiveSearch {
	readonly view: LiveSearchView;
	/** The viewer typed `q` (trimmed): search it once the typing settles. */
	schedule(q: string): void;
	/** The dropdown closed to navigate: nothing more is searched. */
	close(): void;
	/** The layout has gone. */
	destroy(): void;
}

export function createLiveSearch(deps: LiveSearchDeps): LiveSearch {
	// Arrows, not the bare globals: a web timer called as a method of
	// another object throws in Chromium.
	const schedule = deps.schedule ?? ((run, ms) => setTimeout(run, ms));
	const cancel = deps.cancel ?? ((handle) => clearTimeout(handle as ReturnType<typeof setTimeout>));
	const searches = visitSeries();
	let view: LiveSearchView = { results: null, busy: false, error: false };
	/** The query `view.results` answers. */
	let resultsFor: string | null = null;
	let timer: unknown = null;

	const set = (patch: Partial<LiveSearchView>) => {
		view = { ...view, ...patch };
		deps.changed(view);
	};

	/** No search goes on: none waiting on its pause, none out. */
	const stop = () => {
		if (timer !== null) cancel(timer);
		timer = null;
		searches.end();
	};

	const run = async (q: string) => {
		timer = null;
		set({ busy: true, error: false });
		const search = searches.next();
		try {
			const hits = await search.keep(deps.search(q, search.signal));
			if (q !== deps.currentQuery()) return;
			const filtered = await search.keep(deps.filter(hits.slice(0, deps.maxHits), search.signal));
			// The viewer may have typed on while the filter ran.
			if (q !== deps.currentQuery()) return;
			resultsFor = q;
			set({ results: filtered });
		} catch {
			resultsFor = q;
			set({ results: [], error: true });
		} finally {
			set({ busy: false });
		}
	};

	return {
		get view() {
			return view;
		},
		schedule(q) {
			stop();
			if (q.length < deps.minChars) {
				resultsFor = null;
				set({ results: null, busy: false, error: false });
				return;
			}
			timer = schedule(() => void run(q), deps.debounceMs);
		},
		close() {
			stop();
			if (resultsFor === deps.currentQuery()) set({ busy: false });
			else set({ results: null, busy: false, error: false });
		},
		destroy: stop
	};
}
