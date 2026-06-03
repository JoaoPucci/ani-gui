import type { HistoryEntry, KitsuAnimeRef } from '$lib/api';

export interface ContinueWatchingState {
	matches: Record<string, KitsuAnimeRef | null>;
	playableCounts: Record<string, number>;
}

export interface ContinueWatchingLoaderDeps {
	resolveMatch: (entry: HistoryEntry) => Promise<KitsuAnimeRef | null>;
	/**
	 * Per-row availability lookup — same `checkAvailability` the detail
	 * page issues for `playableEpisodeCount`. Already cache-first on
	 * the backend (SQLite hit → fast; cache miss → live allmanga
	 * probe). Drop-in replacement for what was a batch + probe split:
	 * the per-row contract removes the slowest-match gate that the
	 * batch previously introduced.
	 */
	fetchAvailability: (
		match: KitsuAnimeRef,
		mode: 'sub' | 'dub'
	) => Promise<{ episode_count: number | null } | null>;
	/**
	 * Resolves to the configured availability mode. Async because the
	 * home page bootstraps settingsGet() in parallel with historyList()
	 * — the loader must hold on per-row probes until the configured
	 * mode is known, otherwise it would read the wrong (sub vs. dub)
	 * playable count while startResume later uses the loaded mode.
	 */
	getMode: () => Promise<'sub' | 'dub'>;
	/**
	 * Fired per entry as that row's match AND playable count both
	 * become known. The page uses this to write its historyMatches /
	 * historyPlayableCounts maps incrementally so a fast row's card
	 * isn't held behind a slow row's match. No-match rows release
	 * immediately with `(null, null)`. Optional — callers that just
	 * want the final maps (tests, headless consumers) can skip it.
	 */
	onRowReady?: (entryId: string, match: KitsuAnimeRef | null, playableCount: number | null) => void;
	/**
	 * Max concurrent live probes. allmanga is rate-limited, and the
	 * backend's `warm` path spaces equivalent probes by 500ms while
	 * `filterAvailableStrict` caps inline probes at 4. Default 4 here
	 * matches both. Bumping it speeds up reveal for users with many
	 * cards at the cost of higher allmanga load.
	 */
	probeConcurrency?: number;
}

/**
 * Loads the home page's Continue Watching state with per-row release
 * semantics. Each row's match + playable count land together (no
 * stale-cap race), but rows don't gate each other — a slow Kitsu
 * resolution doesn't hold up cards whose data is already in.
 *
 * Pipeline per row:
 *   1. resolveMatch — kitsuSearch + pickKitsuMatch (cache-first at
 *      30d TTL; usually instant on warm runs).
 *   2. If null match: fire onRowReady(null, null) immediately. The
 *      page renders the /search fallback link.
 *   3. Otherwise: await `getMode` (shared promise, awaited once for
 *      the whole load), then enqueue a checkAvailability probe.
 *   4. Probes drain through a bounded worker pool (default 4
 *      concurrent). As each row's probe lands, fire onRowReady
 *      with the resolved count (or null on rejection / no count).
 *
 * The page's button-enable gate reads `historyMatches[entry.id]` —
 * which is only written from inside onRowReady — so a card never
 * flips to its resumable form before its cap is in.
 *
 * The returned `{matches, playableCounts}` is the cumulative view,
 * useful for callers that don't want to track callbacks (tests, the
 * page's defensive "in case onRowReady fires after teardown" guard).
 *
 * Failure modes:
 *   - resolveMatch rejects → entry gets `null` match; onRowReady
 *     fires with (null, null).
 *   - getMode rejects → defaults to `sub`, same fallback the page
 *     uses today.
 *   - probe rejects or returns null/null-count → onRowReady fires
 *     with `(match, null)`; per-card cap then falls back to
 *     match.episode_count via the page's `playableCount ??
 *     match?.episode_count` precedence.
 */
export async function loadContinueWatchingState(
	history: HistoryEntry[],
	deps: ContinueWatchingLoaderDeps
): Promise<ContinueWatchingState> {
	const concurrency = deps.probeConcurrency ?? 4;
	const matches: Record<string, KitsuAnimeRef | null> = {};
	const playableCounts: Record<string, number> = {};
	const modePromise = deps.getMode().catch(() => 'sub' as const);

	const queue: { entry: HistoryEntry; match: KitsuAnimeRef }[] = [];
	let drainResolve!: () => void;
	const drainSignal = new Promise<void>((resolve) => {
		drainResolve = resolve;
	});
	let pendingProbes = 0;
	let matchesPending = history.length;

	const finalizeRow = (entryId: string, match: KitsuAnimeRef | null, count: number | null) => {
		matches[entryId] = match;
		if (typeof count === 'number') playableCounts[entryId] = count;
		deps.onRowReady?.(entryId, match, count);
	};

	const maybeFinishLoad = () => {
		if (matchesPending === 0 && pendingProbes === 0 && queue.length === 0) {
			drainResolve();
		}
	};

	const runProbe = async () => {
		while (queue.length > 0) {
			const job = queue.shift();
			if (!job) break;
			const mode = await modePromise;
			let count: number | null;
			try {
				const r = await deps.fetchAvailability(job.match, mode);
				count = r?.episode_count ?? null;
			} catch {
				count = null;
			}
			finalizeRow(job.entry.id, job.match, count);
			pendingProbes--;
			maybeFinishLoad();
		}
	};

	let workersActive = 0;
	const ensureWorkers = () => {
		while (workersActive < concurrency && queue.length > 0) {
			workersActive++;
			void runProbe().finally(() => {
				workersActive--;
			});
		}
	};

	history.forEach((entry) => {
		deps
			.resolveMatch(entry)
			.then((match) => {
				if (!match) {
					finalizeRow(entry.id, null, null);
				} else {
					pendingProbes++;
					queue.push({ entry, match });
					ensureWorkers();
				}
			})
			.catch(() => {
				finalizeRow(entry.id, null, null);
			})
			.finally(() => {
				matchesPending--;
				maybeFinishLoad();
			});
	});

	if (history.length === 0) drainResolve();
	await drainSignal;
	return { matches, playableCounts };
}
