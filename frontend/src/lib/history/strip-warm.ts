// Warming a Continue Watching strip's AniList fills up front.
//
// Each card asks for its episodes, and its show's detail, only once
// its own row is ready, and on a cold cache rows land seconds apart:
// the backend would then fill each card's stills and banner from
// AniList on its own. Most history rows record the Kitsu id of the
// show the user played, so as soon as the history is loaded Home hands
// the backend those ids at once, and the backend asks AniList for the
// whole strip together; a card that asks later finds its answer kept,
// or shares the lookup still out.

import type { HistoryEntry } from '$lib/api';

/** The most ids one warm carries: one page of AniList's batch. */
export const STRIP_WARM_MAX = 50;

/** The Kitsu ids the strip's rows recorded, in strip order, each once,
 *  at most {@link STRIP_WARM_MAX}. Rows without one are matched by
 *  title and fill on their own. */
export function stripWarmIds(history: HistoryEntry[]): string[] {
	const ids: string[] = [];
	for (const entry of history) {
		const id = entry.kitsu_id;
		if (!id || ids.includes(id)) continue;
		ids.push(id);
		if (ids.length === STRIP_WARM_MAX) break;
	}
	return ids;
}

export interface StripWarmDeps {
	/** The backend's warm (`continueWarm` in `$lib/api`). */
	warm: (ids: string[], opts: { signal?: AbortSignal }) => Promise<void>;
	/** Home's reads: leaving the page drops the warm. */
	signal?: AbortSignal;
}

/** How a warm ended for the cards waiting on it: `answered` once the
 *  backend has filled what it could (a show AniList held back included),
 *  `failed` when the request failed or Home left. */
export type WarmOutcome = 'answered' | 'failed';

/** A warm on its way: the shows it carries and how it ends. */
export interface WarmInFlight {
	ids: ReadonlySet<string>;
	settled: Promise<WarmOutcome>;
}

/** Ask the backend once for the whole strip. A warm that fails or is
 *  dropped leaves each card to fill on its own, as before; the answer
 *  says which shows it carried and how it ended, for the cards'
 *  episode wait ({@link createWarmWatch}). Null when no row recorded a
 *  Kitsu id. */
export function warmContinueStrip(
	history: HistoryEntry[],
	deps: StripWarmDeps
): WarmInFlight | null {
	const ids = stripWarmIds(history);
	if (ids.length === 0) return null;
	const settled = deps.warm(ids, { signal: deps.signal }).then(
		(): WarmOutcome => 'answered',
		(): WarmOutcome => 'failed'
	);
	return { ids: new Set(ids), settled };
}

/** The warms Home has sent this visit, for a card to ask whether one
 *  is still out for its show. */
export function createWarmWatch() {
	return {
		/** Note a warm Home just sent. */
		track(warm: WarmInFlight | null): void {
			void warm;
		},
		/** How the warm still out for `kitsuId` ends, or null when none
		 *  is: the show was in no warm, or its warm has ended. */
		pendingFor(kitsuId: string): Promise<WarmOutcome> | null {
			void kitsuId;
			return null;
		}
	};
}
