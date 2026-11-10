/**
 * The backend's lanes: the loopback origins it listens on, one per
 * class of renderer traffic (the backend's `lanes` module).
 *
 * Chromium opens at most six connections at a time to one host and
 * port over plain HTTP and queues every request past the sixth,
 * whatever it is for. Requests that hold a socket for as long as a
 * provider or an upstream takes — an availability probe, a play
 * resolution, a download's progress stream, a cold poster — would
 * otherwise share those six with the API's short calls and with the
 * player, and starve them. Each lane is an origin of its own and so a
 * connection budget of its own:
 *
 *   - `api`      — everything not named below: short calls, and two
 *                  kinds of long-lived request. The renderer's single
 *                  wait on held-back fills (`$lib/refill/refill`), open
 *                  up to 20 s at a time while a page waits on one, holds
 *                  one of the six, never more. A franchise read stays
 *                  open while the backend walks the franchise, several
 *                  rounds of Kitsu reads when Kitsu is slow, and the
 *                  page that asked cancels it when the viewer leaves.
 *                  The player and the franchise page hold at most one
 *                  (`$lib/watch-order/loader`); the show page holds one
 *                  and the walk's first hop beside it, which waits on
 *                  Kitsu too (`kitsuFranchiseFirstHop`); Home holds one
 *                  per Continue row at its finale, all at once
 *                  (`$lib/watch-order/handoff-reads`).
 *   - `stream`   — the stream proxy; what the player waits on.
 *   - `image`    — `/api/image`, an upstream fetch on a cold cache.
 *   - `probe`    — availability probes, a provider walk each.
 *   - `resolve`  — play resolutions, the user's own and the warms.
 *   - `download` — downloads' progress streams, open for the whole
 *                  download.
 *
 * Every lane serves every route, so a lane nobody announced falls
 * back to the API's origin and still answers — it just shares the
 * API's budget, as everything did before lanes.
 */

/** Every lane the backend announces. */
export const LANES = ['api', 'stream', 'image', 'probe', 'resolve', 'download'] as const;

export type Lane = (typeof LANES)[number];

const trim = (base: string) => base.replace(/\/+$/, '');

/** The origin `lane` is reached at: its own when `lanes` carries one,
 *  the API's otherwise. */
export function laneBase(
	lanes: Partial<Record<string, string>> | undefined,
	lane: Lane,
	apiBase: string
): string {
	const own = lane === 'api' ? undefined : lanes?.[lane];
	return trim(typeof own === 'string' && own.length > 0 ? own : apiBase);
}
