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
	/**
	 * Live availability probe — same call the detail page issues to
	 * compute `playableEpisodeCount`. Fired for matches the batch
	 * didn't cover (CLI-imported history, expired 24h positive
	 * cache). Returning null (or rejecting) is fine: the per-card
	 * cap then falls back to `match.episode_count`.
	 */
	fetchAvailability: (
		match: KitsuAnimeRef,
		mode: 'sub' | 'dub'
	) => Promise<{ episode_count: number | null } | null>;
	/**
	 * Resolves to the configured availability mode. Async because the
	 * home page bootstraps settingsGet() in parallel with historyList()
	 * — the loader must hold on the batch call until the configured
	 * mode is known, otherwise it would read the wrong (sub vs. dub)
	 * playable counts while startResume later uses the loaded mode.
	 */
	getMode: () => Promise<'sub' | 'dub'>;
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
 * Mode wait: `getMode` is awaited concurrently with the per-entry
 * match resolution. The batch is only issued once the configured
 * mode is known — so when the user's saved mode is `dub` but
 * history settled before settings, the batch still reads `dub`
 * playable counts rather than the page-default `sub`.
 *
 * Cache-miss fallback: when the batch returns no entry for a
 * resolved match (CLI-imported history that never went through the
 * list-view warm, expired 24h positive cache on an ongoing show),
 * `fetchAvailability` is fired for that match — the same live
 * probe the detail page does — so the home cap matches detail
 * even in the divergent case. Probes run concurrently; the slowest
 * gates the strip, but the cache hit rate is high in practice so
 * this only fires for genuine misses.
 *
 * Failure modes:
 *   - per-entry resolveMatch rejects → that entry gets `null` match;
 *     no throw escapes.
 *   - batch rejects (cache miss / network blip) → batchCounts is
 *     empty; every match falls through to the live probe.
 *   - live probe rejects or returns null → that entry's playable
 *     count is omitted; per-card cap then falls back to
 *     `match.episode_count`.
 *   - getMode rejects → defaults to `sub`, same fallback the page
 *     uses today.
 *   - no entry matches → batch is skipped (nothing to look up).
 */
export async function loadContinueWatchingState(
	history: HistoryEntry[],
	deps: ContinueWatchingLoaderDeps
): Promise<ContinueWatchingState> {
	const [settled, mode] = await Promise.all([
		Promise.all(
			history.map((entry) =>
				deps
					.resolveMatch(entry)
					.then((match) => ({ entry, match }))
					.catch(() => ({ entry, match: null as KitsuAnimeRef | null }))
			)
		),
		deps.getMode().catch(() => 'sub' as const)
	]);

	const ids = settled.map(({ match }) => match?.id).filter((id): id is string => Boolean(id));

	let batchCounts: Record<string, number> = {};
	if (ids.length > 0) {
		try {
			const r = await deps.fetchAvailabilityBatch(ids, mode);
			batchCounts = r.playable_episode_counts ?? {};
		} catch {
			batchCounts = {};
		}
	}

	// Cache-miss fallback. For each match the batch didn't cover, fire
	// the live probe — same checkAvailability the detail page uses to
	// derive `playableEpisodeCount`. Concurrent; cached rows stay
	// untouched (no wasted IPC).
	const probeSubjects = settled
		.map(({ match }) => match)
		.filter((m): m is KitsuAnimeRef => Boolean(m) && batchCounts[m!.id] === undefined);
	const probeResults = await Promise.all(
		probeSubjects.map((match) =>
			deps
				.fetchAvailability(match, mode)
				.then((r) => ({ id: match.id, count: r?.episode_count ?? null }))
				.catch(() => ({ id: match.id, count: null as number | null }))
		)
	);
	const probeCounts: Record<string, number> = {};
	for (const { id, count } of probeResults) {
		if (typeof count === 'number') probeCounts[id] = count;
	}
	const allCounts: Record<string, number> = { ...batchCounts, ...probeCounts };

	const matches: Record<string, KitsuAnimeRef | null> = {};
	const playableCounts: Record<string, number> = {};
	for (const { entry, match } of settled) {
		matches[entry.id] = match;
		if (match) {
			const c = allCounts[match.id];
			if (typeof c === 'number') playableCounts[entry.id] = c;
		}
	}
	return { matches, playableCounts };
}
