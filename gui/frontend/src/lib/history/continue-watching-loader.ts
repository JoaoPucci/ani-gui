import type { HistoryEntry, KitsuAnimeRef } from '$lib/api';

export interface ContinueWatchingState {
	matches: Record<string, KitsuAnimeRef | null>;
	playableCounts: Record<string, number>;
}

export interface ContinueWatchingLoaderDeps {
	resolveMatch: (entry: HistoryEntry) => Promise<KitsuAnimeRef | null>;
	fetchAvailabilityBatch: (
		ids: string[],
		mode: 'sub' | 'dub'
	) => Promise<{ playable_episode_counts: Record<string, number> }>;
	mode: 'sub' | 'dub';
}

/**
 * Loads the home page's Continue Watching state in a single atomic
 * step: per-entry Kitsu match resolution PLUS the shared
 * availability-batch lookup for playable episode counts. Returns
 * `{matches, playableCounts}` only after both stages settle, so the
 * page can swap both maps into state in lockstep and the card never
 * appears resumable with a stale episode cap visible.
 *
 * Why the wait: the detail page derives its `episodeCap` from the
 * allmanga playable count (more authoritative than Kitsu's
 * announced total — Kitsu often lags by a few episodes on ongoing
 * shows). The home Continue card has to use the same cap, but the
 * batch read can't fire until every per-entry match has resolved
 * (we need the Kitsu ids to query). Writing matches incrementally
 * would briefly let the card derive `nextEpisode` from
 * `match.episode_count` — at-cap rows would replay the last
 * episode from home while the detail page advances. Holding both
 * writes until the batch returns closes that window.
 *
 * Failure modes:
 *   - per-entry resolveMatch rejects → that entry gets `null` match;
 *     no throw escapes.
 *   - batch rejects (cache miss / network blip) → playableCounts is
 *     empty; matches still flow through, so cards still render. Per-
 *     card cap then falls back to `match.episode_count` — same
 *     behaviour as before, just no longer racy.
 *   - no entry matches → batch is skipped (nothing to look up).
 */
export async function loadContinueWatchingState(
	history: HistoryEntry[],
	deps: ContinueWatchingLoaderDeps
): Promise<ContinueWatchingState> {
	const settled = await Promise.all(
		history.map((entry) =>
			deps
				.resolveMatch(entry)
				.then((match) => ({ entry, match }))
				.catch(() => ({ entry, match: null as KitsuAnimeRef | null }))
		)
	);

	const ids = settled.map(({ match }) => match?.id).filter((id): id is string => Boolean(id));

	let counts: Record<string, number> = {};
	if (ids.length > 0) {
		try {
			const r = await deps.fetchAvailabilityBatch(ids, deps.mode);
			counts = r.playable_episode_counts ?? {};
		} catch {
			counts = {};
		}
	}

	const matches: Record<string, KitsuAnimeRef | null> = {};
	const playableCounts: Record<string, number> = {};
	for (const { entry, match } of settled) {
		matches[entry.id] = match;
		if (match) {
			const c = counts[match.id];
			if (typeof c === 'number') playableCounts[entry.id] = c;
		}
	}
	return { matches, playableCounts };
}
