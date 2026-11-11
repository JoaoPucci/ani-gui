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
 *   - `api`      — everything not named below: short calls, and one
 *                  long-lived request. The renderer's single wait on
 *                  held-back fills (`$lib/refill/refill`), open up to
 *                  20 s at a time while a page waits on one, holds one
 *                  of the six, never more.
 *   - `stream`   — the stream proxy; what the player waits on.
 *   - `image`    — `/api/image`, an upstream fetch on a cold cache.
 *   - `probe`    — availability probes, a provider walk each.
 *   - `resolve`  — play resolutions, the user's own and the warms.
 *   - `download` — downloads' progress streams, open for the whole
 *                  download.
 *   - `franchise` — franchise reads and the walk's first hop
 *                  (`kitsuFranchise`, `kitsuFranchiseFirstHop`), each
 *                  open while the backend walks the franchise, several
 *                  rounds of Kitsu reads when Kitsu is slow; the page
 *                  that asked cancels it when the viewer leaves. The
 *                  show page holds at most two, its read and the first
 *                  hop, cancelled with the visit
 *                  (`$lib/watch-order/loader`). The player and the
 *                  franchise page hold at most one each, a superseded
 *                  read cancelled (`$lib/watch-order/latest-read`). Home
 *                  walks the series of its Continue rows at their finale
 *                  six at a time, the lane's budget, and the rest in
 *                  turn (`$lib/watch-order/handoff-reads`).
 *
 * Every lane serves every route, so a lane nobody announced falls
 * back to the API's origin and still answers — it just shares the
 * API's budget, as everything did before lanes.
 */

/** Every lane the backend announces. */
export const LANES = [
	'api',
	'stream',
	'image',
	'probe',
	'resolve',
	'download',
	'franchise'
] as const;

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
