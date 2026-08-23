/**
 * In-memory Kitsu-episode-page cache used by /anime/[id] and
 * /play/[id]. Keyed by Kitsu page number (1, 2, ...), value is the
 * page's KitsuEpisode array as returned by `kitsuEpisodes(id, p)`.
 *
 * Why this module exists: SvelteKit reuses the same route component
 * instance when the user navigates between two URLs that match the
 * same dynamic route (e.g. /anime/A → /anime/B via a similar-titles
 * card). The route's id-change reset nulls `episodes`, `detail`,
 * `similar`, etc., but a bare `new SvelteMap()` declared in
 * component scope is easy to forget. Routing reads/writes through
 * `resetEpisodePageCache` so the contract — "page-cache is per-
 * show, drop it when the show changes" — lives in one place.
 *
 * SvelteMap (vs plain Map) keeps the svelte/prefer-svelte-reactivity
 * eslint rule happy when the cache is touched from $derived/$effect.
 */
import { SvelteMap } from 'svelte/reactivity';
import type { KitsuEpisode } from '$lib/api';

export type EpisodePageCache = SvelteMap<number, KitsuEpisode[]>;

export function createEpisodePageCache(): EpisodePageCache {
	return new SvelteMap<number, KitsuEpisode[]>();
}

export function resetEpisodePageCache(cache: EpisodePageCache): void {
	cache.clear();
}
