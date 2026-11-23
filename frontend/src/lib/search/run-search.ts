/**
 * The /search page's query flow, extracted per AGENTS.md §2 so the
 * component stays a thin adapter. One runner instance owns the
 * page's async state transitions: config load, the Kitsu search a
 * page at a time, and the progressive availability filter's
 * emissions for each page.
 *
 * Every run is its own search, and a superseded one is silenced by
 * identity, NOT by comparing query text: navigating A → B → A starts
 * two distinct searches for the text "A", and a text guard would let
 * the first one's late emissions — or a later page of it still in
 * flight — overwrite the second's results and clear its spinner
 * early. Every callback checks that its search is still the current
 * one before touching page state.
 *
 * Paging: each page goes through the filter on its own and renders
 * as its own slice of the grid, so a late prune on one page never
 * disturbs another. What follows a page's first render — settle,
 * end, load the next page by itself, or hold for the user — is the
 * pure rule in `paging.ts`.
 *
 * The engine (the current search, fetching and settling a page,
 * starting a search that replaces the current one) is `search-core.ts`;
 * this module composes the page's operations from it.
 */

import type { SearchOrder } from '$lib/api';
import { canLoadMore } from './paging';
import type { ProbeRun } from '$lib/availability/probe-scope';
import type { SearchCheckpoint, SearchSnapshot } from './snapshot';
import { anchorShown, checkpointOf, restoredState, resumePlan } from './resume';
import { createSearchCore, type SearchRunnerDeps } from './search-core';

export type { SearchRunnerDeps } from './search-core';

export function createSearchRunner(deps: SearchRunnerDeps): {
	/** Start a search for `q` in `order` (Kitsu's ranking by default). */
	run: (q: string, order?: SearchOrder) => Promise<void>;
	/** Load the next page: when idle, retrying a failed page, or
	 *  continuing a held search. Does nothing in any other phase. */
	loadMore: () => void;
	/** Where the current search stood when its last page settled, with
	 *  the results it shows; `null` before a first page has settled. */
	checkpoint: () => SearchCheckpoint | null;
	/** End the current search: nothing more of it reaches the page. */
	stop: () => void;
	/** Bring back a search the page saved when the reader left it. */
	resume: (saved: SearchSnapshot, at: { now: number; reload: boolean }) => Promise<void>;
} {
	const core = createSearchCore(deps);
	const { state, launch, begin, fetchPage, fetchMore, setPhase } = core;

	const run = (q: string, order: SearchOrder = 'relevance') =>
		launch((probes) => fetchPage(begin(q, order, probes, null)));

	const checkpoint = (): SearchCheckpoint | null => checkpointOf(state.search);

	/** The saved search made the current one as it was, its results
	 *  shown without asking Kitsu for anything. */
	const restore = (saved: SearchSnapshot, kept: SearchCheckpoint, probes: ProbeRun) => {
		Object.assign(begin(saved.q, saved.order, probes, null), restoredState(kept));
		deps.onResults(kept.hits);
		setPhase({ kind: kept.phase });
		deps.onReached?.(anchorShown(saved.anchor, kept.hits));
	};

	/**
	 * Bring back a saved search: as it was, when `canReuse` allows;
	 * otherwise run again in the saved order, loading pages until the
	 * card the reader left at the top shows or the pages they had are
	 * loaded. Either way `onReached` says whether that card is up.
	 */
	const resume = (saved: SearchSnapshot, at: { now: number; reload: boolean }) =>
		launch((probes) => {
			const plan = resumePlan(saved, { ...at, mode: deps.pickMode(core.settings.config) });
			if ('kept' in plan) return restore(saved, plan.kept, probes);
			return fetchPage(begin(saved.q, saved.order, probes, plan.target));
		});

	const loadMore = () => {
		const { search, phase } = state;
		if (!search || !canLoadMore(phase)) return;
		if (phase.kind === 'held') search.emptyStreak = 0;
		void fetchMore(search);
	};

	const stop = () => {
		core.turns.end();
		state.search = null;
		deps.onBusy(false);
	};

	return { run, loadMore, checkpoint, resume, stop };
}
