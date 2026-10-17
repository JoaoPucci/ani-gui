/**
 * Pure helpers for the `/anime/[id]` page's URL deep-link effects.
 * The detail page opens on the episode page a `?page=N` names (no
 * link in the app builds one now; a bookmarked or pasted URL can
 * carry it); this module owns the parsing + decision rules so the
 * `$effect` blocks in the component become thin glue.
 *
 * Companion to `lib/history/nav-depth.ts` — same extraction pattern.
 */

/** Coerce a `?page=` query param into a 1-based UI page. Falls back
 *  to `1` for missing / non-numeric / non-positive values. */
export function parsePageParam(searchParams: URLSearchParams): number {
	const raw = searchParams.get('page') ?? '';
	const n = parseInt(raw, 10);
	return Number.isFinite(n) && n > 0 ? n : 1;
}

/** What the page-driving `$effect` should do given current state +
 *  the URL-derived target page. */
export type EpisodeFetchAction = 'fetch-initial' | 'fetch' | 'noop';

export function decideEpisodeFetchAction(opts: {
	episodes: readonly unknown[] | null;
	episodesPage: number;
	episodesLoading: boolean;
	targetPage: number;
}): EpisodeFetchAction {
	if (opts.episodes === null) return 'fetch-initial';
	if (opts.targetPage === opts.episodesPage) return 'noop';
	if (opts.episodesLoading) return 'noop';
	return 'fetch';
}
