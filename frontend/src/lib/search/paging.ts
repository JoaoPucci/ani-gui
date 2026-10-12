/**
 * The pure rules behind the search page's infinite scroll: what state
 * the paging is in, what follows a page's first render, which hits a
 * page may add, and when the bottom of the grid asks for more. The
 * runner (`run-search.ts`) owns the async flow and applies these.
 */

import type { KitsuAnimeRef } from '$lib/api';

/** How many pages in a row may come back with nothing to show before
 *  the runner stops fetching on its own and waits for the user. */
export const MAX_EMPTY_PAGES = 3;

/**
 * - `loading`: the first page is in flight.
 * - `idle`: more pages exist; the next loads when asked for.
 * - `loading-more`: a later page is in flight.
 * - `end`: Kitsu has no more pages.
 * - `failed`: a later page failed; asking again retries it.
 * - `held`: {@link MAX_EMPTY_PAGES} pages in a row had nothing to
 *   show; asking again keeps looking.
 */
export type PagingPhase =
	| { kind: 'loading' }
	| { kind: 'idle' }
	| { kind: 'loading-more' }
	| { kind: 'end' }
	| { kind: 'failed'; error: unknown }
	| { kind: 'held' };

/** What follows a page's first render. */
export interface AfterPage {
	phase: PagingPhase;
	/** Pages in a row, this one included, that had nothing to show. */
	emptyStreak: number;
	/** Whether the next page loads now, without being asked for. */
	advance: boolean;
}

export function afterPage(
	visible: number,
	nextOffset: number | null,
	emptyStreak: number
): AfterPage {
	if (visible > 0) {
		return {
			phase: { kind: nextOffset === null ? 'end' : 'idle' },
			emptyStreak: 0,
			advance: false
		};
	}
	if (nextOffset === null) return { phase: { kind: 'end' }, emptyStreak, advance: false };
	const streak = emptyStreak + 1;
	if (streak >= MAX_EMPTY_PAGES)
		return { phase: { kind: 'held' }, emptyStreak: streak, advance: false };
	return { phase: { kind: 'loading-more' }, emptyStreak: streak, advance: true };
}

/** The hits of `hits` not already in `seen`, recording them there.
 *  Kitsu's ranking can shift between two page requests, so a show can
 *  turn up on two pages; the first appearance is the one kept. */
export function freshHits(seen: Set<string>, hits: KitsuAnimeRef[]): KitsuAnimeRef[] {
	return hits.filter((h) => {
		if (seen.has(h.id)) return false;
		seen.add(h.id);
		return true;
	});
}

/** Whether asking for more does anything in this phase. */
export function canLoadMore(phase: PagingPhase): boolean {
	return phase.kind === 'idle' || phase.kind === 'failed' || phase.kind === 'held';
}

/** What the page knows about the marker under the grid: whether its
 *  latest measurement put it in reach, and whether that measurement is
 *  still fresh — taken since the last page settled and not yet used to
 *  load one. */
export interface Reach {
	inReach: boolean;
	fresh: boolean;
}

/** Nothing measured yet: a new search, or a marker just unmounted. */
export const UNMEASURED: Reach = { inReach: false, fresh: false };

/** A new measurement from the marker's observer. */
export function measured(inReach: boolean): Reach {
	return { inReach, fresh: true };
}

/** The measurement used up: by the load it started, or by a page
 *  settling, which moves the marker the measurement was taken at. The
 *  next load by itself waits for a measurement taken after that. */
export function spent(reach: Reach): Reach {
	return { inReach: reach.inReach, fresh: false };
}

/** Whether the bottom of the grid coming into view loads the next page
 *  by itself. Only an idle search does: a failed or held one waits for
 *  the user's own action, so a page that keeps failing cannot spin.
 *  Nor does a `narrowed` grid — one a filter on the page hides part
 *  of: it can stay shorter than the screen, keeping the bottom in
 *  reach after every page, and its pages are not empty, only hidden,
 *  so the empty-page hold would never stop the walk. And only on a
 *  fresh measurement: one taken before the last page settled says
 *  where the marker was, not where it is. */
export function shouldAutoLoad(phase: PagingPhase, reach: Reach, narrowed: boolean): boolean {
	return !narrowed && reach.inReach && reach.fresh && phase.kind === 'idle';
}
