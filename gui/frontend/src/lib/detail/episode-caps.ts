/**
 * Episode-count caps and clamps layered on the airing schedule.
 *
 * Companion to ./episode-airing (per-tile state): these helpers
 * decide how far the strip renders and how far actions may reach —
 * split into their own module so each file's total complexity stays
 * under the CRAP ratchet's high-risk line.
 */

import { epAirState, type AiringStatus } from './episode-airing';

/**
 * True when episode `n` sits above allmanga's playable count — aired
 * per AniList, but not streamable yet (catalog lag: aired=5 while
 * allmanga lists 2). `displayCap` renders such tiles; clicks and
 * prefetches must not fire on them (Codex P2 #3565988141/#3565988143).
 * Floor-compare mirrors `epAirState`: decimal extras come from
 * allmanga itself, so a streamable "2.5" recap isn't gated by a
 * playable count of 2. Unknown count never gates.
 */
export function beyondPlayable(n: number, playable: number | null): boolean {
	if (playable === null) return false;
	return Math.floor(n) > playable;
}

/**
 * How far the episode strip should render tiles. Actions (play,
 * download, prefetch, next/prev) keep the playable-first cap; this
 * governs *display only*. With airing data present, extend to the
 * announced total so a season allmanga hasn't fully listed renders
 * its unaired tail as greyed dated tiles instead of not existing at
 * all. Without airing data nothing could grey that tail — padded
 * extras would be interactive doomed tiles — so stick to the
 * playable-first cap.
 */
export function displayCap(
	playable: number | null,
	announced: number | null,
	airing: AiringStatus | null
): number | null {
	const base = playable ?? announced;
	if (airing == null || airing.aired == null) return base;
	if (playable === null || announced === null) return base;
	return Math.max(playable, announced);
}

/**
 * Clamp an episode cap to the aired count. The primary Play/Continue
 * CTA computes its target as `pickNextEpisode(last, cap)` — without
 * the clamp, a user watched through the aired count gets "Continue"
 * into an unaired episode, the same doomed resolution the tiles
 * disable (Codex P2 #3565649454). Unknown airing data passes the cap
 * through untouched.
 */
export function airedCap(cap: number | null, airing: AiringStatus | null): number | null {
	const aired = airing?.aired ?? null;
	if (aired === null) return cap;
	return cap === null ? aired : Math.min(cap, aired);
}

/**
 * Filter a prefetch target list down to aired episodes. The detail
 * page's background warm must not spend scraper slots resolving
 * greyed-out future episodes (Codex P2 #3565590966); unknown airing
 * data passes everything through, mirroring `epAirState`.
 */
export function airedTargets(targets: number[], airing: AiringStatus | null): number[] {
	return targets.filter((n) => !epAirState(n, airing).unaired);
}
