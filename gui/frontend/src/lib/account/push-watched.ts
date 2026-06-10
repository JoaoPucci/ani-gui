// Fan-out trigger for write-back: on mark-watched, push the episode
// the user just watched to every connected tracker. Renderer-driven
// (the backend is stateless) — we hold the per-provider bearers, so
// the renderer orchestrates the calls. Green commit implements.

import type { ListEntry, Provider } from './types';

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

export async function pushWatchedToTrackers(
	deps: PushWatchedDeps,
	kitsuId: string,
	episode: number
): Promise<void> {
	if (!kitsuId || deps.connected.length === 0) return;
	await Promise.all(
		deps.connected.map(async (provider) => {
			const bearer = deps.bearerFor(provider);
			if (!bearer) return;
			try {
				await deps.updateProgress(provider, bearer, {
					kitsu_id: kitsuId,
					progress: episode,
					status: 'watching'
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
export function syncWatchedToTrackers(kitsuId: string, episode: number): Promise<void> {
	// Stub — green commit wires the store deps into pushWatchedToTrackers.
	throw new Error(`syncWatchedToTrackers not implemented (${kitsuId}@${episode})`);
}
