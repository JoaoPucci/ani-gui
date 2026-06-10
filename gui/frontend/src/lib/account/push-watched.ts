// Fan-out trigger for write-back: on mark-watched, push the episode
// the user just watched to every connected tracker. Renderer-driven
// (the backend is stateless) — we hold the per-provider bearers, so
// the renderer orchestrates the calls. Green commit implements.

import type { ListEntry, Provider } from './types';
import { updateProgress } from './api';
import { accountStore } from './store.svelte';
import { bearerFor } from './state-helpers';

export interface PushWatchedDeps {
	/** Providers currently connected (from the account store). */
	connected: Provider[];
	/** Resolve a provider's bearer, or null if unavailable. */
	bearerFor: (provider: Provider) => string | null;
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
 * the user's status. Critically this preserves a `rewatching`/repeating
 * row: sending `watching` would downgrade it (AniList `CURRENT`, MAL
 * `is_rewatching=false`) — Codex P2 #3387319861.
 *
 * `seriesFinished` gates completion (Codex P2 #3387184082): a
 * currently-airing show only ever has the latest released episode, so
 * only a Kitsu `finished` show completes.
 *
 * `seriesTotal` is the show's FULL finite episode count (Kitsu's
 * mode-independent announced total), NOT the mode-specific playable
 * cap — in `dub` mode the playable cap can be a fraction of the series
 * (e.g. 12 dubbed of 24), and watching the last dubbed episode must
 * not complete the whole series (Codex P2 #3387467149). null/0 means
 * the total is unknown — don't complete (Codex P2 #3386988961).
 */
export function watchedStatus(
	episode: number,
	seriesTotal: number | null,
	seriesFinished: boolean
): string | null {
	const atFinale = !!seriesTotal && seriesTotal > 0 && episode >= seriesTotal;
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
	const status = watchedStatus(episode, seriesTotal, seriesFinished);
	const body = status
		? { kitsu_id: kitsuId, progress: episode, status }
		: { kitsu_id: kitsuId, progress: episode };
	await Promise.all(
		deps.connected.map(async (provider) => {
			const bearer = deps.bearerFor(provider);
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
export function syncWatchedToTrackers(
	kitsuId: string,
	episode: number,
	seriesTotal: number | null = null,
	seriesFinished: boolean = false
): Promise<void> {
	return pushWatchedToTrackers(
		{
			connected: accountStore.connected,
			bearerFor: (provider) => bearerFor(accountStore.byProvider[provider]),
			updateProgress
		},
		kitsuId,
		episode,
		seriesTotal,
		seriesFinished
	);
}
