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
	) => Promise<{ episode_count: number | null; episode_count_approximate?: boolean } | null>;
	/**
	 * Resolves to the configured availability mode. Async because the
	 * home page bootstraps settingsGet() in parallel with historyList()
	 * — the loader must hold on per-row probes until the configured
	 * mode is known, otherwise it would read the wrong (sub vs. dub)
	 * playable count while startResume later uses the loaded mode.
	 */
	getMode: () => Promise<'sub' | 'dub'>;
	/**
	 * Fired per entry the moment its match is known — WITHOUT waiting
	 * on the availability probe — and a second time if a probe later
	 * refines the playable cap. The page uses this to write its
	 * historyMatches / historyPlayableCounts maps incrementally: the
	 * first call is what renders the card and enables its click (cap
	 * falls back to match.episode_count), the refinement call tightens
	 * the cap. No-match rows release with `(null, null)`. Optional —
	 * callers that just want the final maps can skip it.
	 */
	onRowReady?: (entryId: string, match: KitsuAnimeRef | null, playableCount: number | null) => void;
	/**
	 * Max concurrent live probes. allmanga is rate-limited, and the
	 * backend's `warm` path spaces equivalent probes by 500ms while
	 * `filterAvailableProgressive` caps inline probes at 4. Default 4
	 * here matches both. Bumping it speeds up reveal for users with
	 * many cards at the cost of higher allmanga load.
	 */
	probeConcurrency?: number;
}

/**
 * Loads the home page's Continue Watching state with render-then-
 * refine semantics: the video site is never on the rendering path.
 * A card needs only its Kitsu match to draw and be clickable, so
 * each row releases the moment resolveMatch lands; the allmanga
 * probe runs behind it and only refines the playable cap. Click
 * safety doesn't depend on the probe — pressing play runs its own
 * live resolution with real error feedback, which is the only
 * answer that's current at the moment it matters.
 *
 * Pipeline per row:
 *   1. resolveMatch — kitsuSearch + pickKitsuMatch (cache-first at
 *      30d TTL; usually instant on warm runs).
 *   2. Null match → onRowReady(null, null); the page renders the
 *      /search fallback link. Otherwise → onRowReady(match, null)
 *      IMMEDIATELY: the card renders now, cap falls back to
 *      match.episode_count.
 *   3. Await `getMode` (shared promise, awaited once for the whole
 *      load), then enqueue a checkAvailability probe.
 *   4. Probes drain through a bounded worker pool (default 4
 *      concurrent). A probe that lands WITH a count fires a second
 *      onRowReady(match, count) so the cap tightens; a countless or
 *      failed probe adds nothing the card doesn't already have and
 *      stays silent — the card keeps its fallback cap either way.
 *
 * The returned `{matches, playableCounts}` is the cumulative view,
 * useful for callers that don't want to track callbacks (tests, the
 * page's defensive "in case onRowReady fires after teardown" guard).
 * The promise still resolves only after every probe settles.
 *
 * Failure modes:
 *   - resolveMatch rejects → entry gets `null` match; onRowReady
 *     fires with (null, null).
 *   - getMode rejects → defaults to `sub`, same fallback the page
 *     uses today.
 *   - probe rejects or returns null/null-count → no second
 *     callback; per-card cap keeps the match.episode_count fallback
 *     via the page's `playableCount ?? match?.episode_count`
 *     precedence.
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
			// Refine only when the probe actually knows something: the
			// card already released at match time with the episode_count
			// fallback, so a countless probe would just re-trigger the
			// row's episode fetch for an identical cap.
			if (typeof count === 'number') {
				finalizeRow(job.entry.id, job.match, count);
			}
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
				// Re-pump after decrement: a row that queued in the gap
				// between this worker's exit-while-loop and now would
				// have been skipped by its own ensureWorkers call
				// (workersActive was still at the cap). Recheck the
				// queue here so the orphan picks up a slot.
				ensureWorkers();
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
					// Release the card now — the Kitsu match is everything
					// a card needs to render and take a click.
					finalizeRow(entry.id, match, null);
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
