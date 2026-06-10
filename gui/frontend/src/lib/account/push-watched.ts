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
	/** POST the update to one provider. */
	updateProgress: (
		provider: Provider,
		bearer: string,
		body: { kitsu_id: string; progress: number; status: string }
	) => Promise<ListEntry | null>;
}

/**
 * Unified status to sync for an episode just watched. The finale of a
 * finite series (episode N of N) moves the tracker to `completed`;
 * everything else stays `watching`. `episodeCount` null/0 means the
 * total is unknown (ongoing show, or Kitsu has no count) — stay
 * `watching` rather than guess. Codex P2 #3386988961.
 */
export function watchedStatus(episode: number, episodeCount: number | null): string {
	return episodeCount && episodeCount > 0 && episode >= episodeCount ? 'completed' : 'watching';
}

export async function pushWatchedToTrackers(
	deps: PushWatchedDeps,
	kitsuId: string,
	episode: number,
	episodeCount: number | null = null
): Promise<void> {
	if (!kitsuId || deps.connected.length === 0) return;
	const status = watchedStatus(episode, episodeCount);
	await Promise.all(
		deps.connected.map(async (provider) => {
			const bearer = deps.bearerFor(provider);
			if (!bearer) return;
			try {
				await deps.updateProgress(provider, bearer, {
					kitsu_id: kitsuId,
					progress: episode,
					status
				});
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
	episodeCount: number | null = null
): Promise<void> {
	return pushWatchedToTrackers(
		{
			connected: accountStore.connected,
			bearerFor: (provider) => bearerFor(accountStore.byProvider[provider]),
			updateProgress
		},
		kitsuId,
		episode,
		episodeCount
	);
}
