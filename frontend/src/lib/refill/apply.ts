/**
 * What a page does with a read fetched once more after its refill
 * landed (`./refill`): take the filled-in parts, and only for the show
 * it still shows. Pure, so the detail and play pages stay thin adapters.
 */
import type { KitsuAnimeRef, KitsuEpisode } from '$lib/api';

/**
 * Put a refilled episode page in `cache` and into the episodes shown.
 * Returns what to show: `shown` with each episode the refill carries
 * replaced by its refilled self. The refill replaces the cached page
 * only when that page is the same show's — it holds one of the refill's
 * episodes; a page the cache no longer holds, or holds for another show
 * the page moved on to, is left as it is, and so is `shown`.
 */
export function refillEpisodes(
	cache: Map<number, KitsuEpisode[]>,
	page: number,
	filled: KitsuEpisode[],
	shown: KitsuEpisode[] | null
): KitsuEpisode[] | null {
	const cached = cache.get(page);
	const ids = new Set(filled.map((ep) => ep.id));
	if (!cached?.some((ep) => ids.has(ep.id))) return shown;
	cache.set(page, filled);
	if (shown === null) return null;
	const byId = new Map(filled.map((ep) => [ep.id, ep]));
	if (!shown.some((ep) => byId.has(ep.id))) return shown;
	return shown.map((ep) => byId.get(ep.id) ?? ep);
}

/** The images a refilled detail brings: its cover and AniList banner. */
export interface RefilledImages {
	cover_image: KitsuAnimeRef['cover_image'];
	anilist_banner: KitsuAnimeRef['anilist_banner'];
}

/**
 * The images to put on `current` from its refilled detail, or `null`
 * when there is nothing to change: no detail shown, a detail of another
 * show, or a refill that brought no cover.
 */
export function refilledImages(
	current: KitsuAnimeRef | null,
	fresh: KitsuAnimeRef
): RefilledImages | null {
	if (current === null || current.id !== fresh.id || fresh.cover_image === null) return null;
	return { cover_image: fresh.cover_image, anilist_banner: fresh.anilist_banner ?? null };
}
