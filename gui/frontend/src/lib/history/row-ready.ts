import type { HistoryEntry, KitsuAnimeRef, KitsuEpisode } from '$lib/api';
import { EPISODES_KITSU_PAGE_SIZE, resolveHistoryEntry } from './resolve';
import { pickNextEpisode } from '$lib/play/next-episode';

export interface ContinueRowReadyDeps {
	/** Snapshot of the current history list, keyed by entry id. The
	 *  handler reads the entry to compute the row's kitsu target; a
	 *  miss means the row rotated out under us (the page navigated
	 *  away or history was refreshed), so the episode fetch is
	 *  skipped — the state write is still safe (it just lands in a
	 *  map the page no longer renders). */
	historyById: Map<string, HistoryEntry>;
	/** IPC wrapper for `/api/kitsu/episodes/<id>?page=N`. Factored out
	 *  so tests can supply a synchronous stub instead of mocking the
	 *  api module. */
	fetchKitsuEpisodes: (kitsuId: string, page: number) => Promise<KitsuEpisode[]>;
	/** Setters for the three pieces of page state the row owns. The
	 *  page side provides one-line Svelte reassignments
	 *  (`historyMatches = { ...historyMatches, [id]: m }` and friends)
	 *  so all reactivity stays component-scoped. */
	setMatch: (entryId: string, match: KitsuAnimeRef | null) => void;
	setPlayableCount: (entryId: string, count: number) => void;
	setEpisode: (entryId: string, episode: KitsuEpisode | null) => void;
}

/**
 * Builds the loader's `onRowReady` callback. The callback handles
 * three concerns the home page's Continue Watching strip ties
 * together when a row's match + playable count both land:
 *
 *   1. Surface the resolved match and (when known) playable count
 *      via the page-side setters. The page reads
 *      `historyMatches[entry.id]` to decide whether the card flips
 *      to its resumable button form, so the match write here is what
 *      gates the click affordance.
 *
 *   2. Decide which Kitsu episode to fetch metadata for. Mirrors the
 *      template's cap rule — `playableCount ?? match.episode_count`
 *      — so the badge thumbnail and canonical title belong to the
 *      episode the click would actually play (pickNextEpisode of
 *      the watched ep against that cap), not the episode the user
 *      just finished. Cour-split shows are routed through
 *      resolveHistoryEntry so the page index resolves to the parent
 *      Kitsu show's per-cour numbering.
 *
 *   3. Stream the episode metadata in via fetchKitsuEpisodes. The
 *      `number` field wins when present; `relative_number` is the
 *      last-resort fallback (Kitsu returns absolute episode numbers
 *      for some multi-cour shows). Either fetch path that doesn't
 *      surface a usable episode degrades to `null`, which the
 *      template renders as the show poster instead of an episode
 *      thumbnail.
 *
 * Lives in $lib so the home component's `<script>` stays a thin
 * adapter — extracted per AGENTS.md §2 ("more than a couple of
 * lines of imperative logic [...] extract it into a sibling .ts
 * module under $lib and unit-test the module").
 */
export function makeContinueRowReadyHandler(
	deps: ContinueRowReadyDeps
): (entryId: string, match: KitsuAnimeRef | null, playableCount: number | null) => void {
	return (entryId, match, playableCount) => {
		deps.setMatch(entryId, match);
		if (typeof playableCount === 'number') {
			deps.setPlayableCount(entryId, playableCount);
		}
		if (!match) return;
		const entry = deps.historyById.get(entryId);
		if (!entry) return;
		const target = resolveHistoryEntry(entry, match);
		// resolveHistoryEntry guarantees kitsuEpisode is a positive
		// number whenever match is non-null (displayEpisode falls back
		// to 1 on parse failure), so no null guard is needed here.
		const cap = playableCount ?? match.episode_count ?? null;
		const nextEpisode = pickNextEpisode(target.kitsuEpisode!, cap);
		const kitsuPage = Math.max(1, Math.ceil(nextEpisode / EPISODES_KITSU_PAGE_SIZE));
		void deps
			.fetchKitsuEpisodes(match.id, kitsuPage)
			.then((eps) => {
				const ep =
					eps.find((e) => e.number === nextEpisode) ??
					eps.find((e) => e.relative_number === nextEpisode) ??
					null;
				deps.setEpisode(entryId, ep);
			})
			.catch(() => {
				deps.setEpisode(entryId, null);
			});
	};
}
