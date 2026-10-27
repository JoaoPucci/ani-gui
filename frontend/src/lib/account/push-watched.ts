// Fan-out trigger for write-back: on mark-watched, push the episode
// the user just watched to every connected tracker. Renderer-driven
// (the backend is stateless) — we hold the per-provider bearers, so
// the renderer orchestrates the calls.

import type { ListEntry, Provider } from './types';
import { updateProgress } from './api';
import { accountStore } from './store.svelte';
import { freshBearerFor } from './fresh-bearer';
import { invalidateWatchLater } from './watch-later-refresh';

export interface PushWatchedDeps {
	/** Providers currently connected (from the account store). */
	connected: Provider[];
	/** Resolve a provider's bearer, or null if unavailable. May be async
	 *  so the caller can refresh a near-expiry token before the write. */
	bearerFor: (provider: Provider) => string | null | Promise<string | null>;
	/** POST the update to one provider. `status` omitted = leave the
	 *  tracker's current status untouched. */
	updateProgress: (
		provider: Provider,
		bearer: string,
		body: { kitsu_id: string; progress: number; status?: string }
	) => Promise<ListEntry | null>;
}

/**
 * Unified status to sync for an episode just watched, or `null` to
 * leave the tracker's existing status untouched.
 *
 * Only the finale of a finished finite series (episode N of N) sets a
 * status — `completed`. Every other progress update returns `null` and
 * sends progress alone, so a normal next-episode write never overrides
 * the user's status. Critically this preserves AniList's `rewatching`
 * (REPEATING) row: sending `watching` would downgrade it to `CURRENT`.
 *
 * `seriesFinished` gates completion: a
 * currently-airing show only ever has the latest released episode, so
 * only a Kitsu `finished` show completes.
 *
 * `seriesTotal` is the show's FULL finite episode count (Kitsu's
 * mode-independent announced total), NOT the mode-specific playable
 * cap — in `dub` mode the playable cap can be a fraction of the series
 * (e.g. 12 dubbed of 24), and watching the last dubbed episode must
 * not complete the whole series. null/0 means
 * the total is unknown — don't complete.
 *
 * The backend reads `completed` on the progress route as this finale
 * and drops it where the tracker's own episode total is known and
 * differs from the episode (and, as before, for an AniList rewatching row,
 * which AniList closes itself; MyAnimeList's rewatching row, a Watching one
 * there, is completed), so
 * nothing else may send `completed` through `updateProgress`.
 */
export function watchedStatus(
	episode: number,
	seriesTotal: number | null,
	seriesFinished: boolean
): string | null {
	// Kitsu's own last episode, not any episode at or past it: one the
	// provider lists past Kitsu's count has no known place in the
	// tracker's numbering, so it completes nothing.
	const atFinale = !!seriesTotal && seriesTotal > 0 && episode === seriesTotal;
	return seriesFinished && atFinale ? 'completed' : null;
}

export async function pushWatchedToTrackers(
	deps: PushWatchedDeps,
	kitsuId: string,
	episode: number,
	seriesTotal: number | null = null,
	seriesFinished: boolean = false
): Promise<void> {
	if (!kitsuId || deps.connected.length === 0) return;
	// Trackers count whole episodes; a fractional extra (a 7.5 recap)
	// is not one of them and moves no progress.
	if (!Number.isInteger(episode)) return;
	const status = watchedStatus(episode, seriesTotal, seriesFinished);
	const body = status
		? { kitsu_id: kitsuId, progress: episode, status }
		: { kitsu_id: kitsuId, progress: episode };
	await Promise.all(
		deps.connected.map(async (provider) => {
			const bearer = await deps.bearerFor(provider);
			if (!bearer) return;
			try {
				await deps.updateProgress(provider, bearer, body);
			} catch {
				// Best-effort: a single tracker failing must not block the
				// others or surface to the caller. Retry/toast deferred.
			}
		})
	);
}

/**
 * Live-store wiring for the play surfaces: fan the just-watched
 * episode out to every connected tracker. Reads the connected
 * providers + their bearers off the account store and delegates to
 * the pure `pushWatchedToTrackers`. Best-effort — safe to `void`.
 */
export async function syncWatchedToTrackers(
	kitsuId: string,
	episode: number,
	seriesTotal: number | null = null,
	seriesFinished: boolean = false
): Promise<void> {
	const connected = accountStore.connected;
	await pushWatchedToTrackers(
		{
			connected,
			// Refresh a near-expiry token before the write so a long-open
			// session doesn't push progress with a stale bearer.
			bearerFor: (provider) => freshBearerFor(provider),
			updateProgress
		},
		kitsuId,
		episode,
		seriesTotal,
		seriesFinished
	);
	// A watched episode can promote a Plan-to-Watch title to Watching,
	// which should drop it from the home Watch Later rail. Invalidate the
	// cached snapshot so the rail re-pulls on its next load (manual
	// refresh or remount) rather than serving a stale, just-watched title
	// out of the freshness TTL.
	for (const provider of connected) invalidateWatchLater(provider);
}
