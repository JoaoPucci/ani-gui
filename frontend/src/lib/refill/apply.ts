/**
 * What a page does with a read fetched once more after its refill
 * landed (`./refill`): take the filled-in parts, and only for the show
 * it still shows. Pure, so the detail and play pages stay thin adapters.
 */
import type { KitsuAnimeRef, KitsuEpisode } from '$lib/api';

/**
 * Fill in the stills a refilled episode page brings — on the cached page
 * and on the episodes shown — in place: each episode keeps its object,
 * and only a still it lacked is set, so Kitsu's own stays and nothing
 * else on the page changes or re-renders. Only when the cached page is
 * the same show's (it holds one of the refill's episodes); a page the
 * cache no longer holds, or holds for another show the page moved on
 * to, is left as it is, and so is `shown`.
 */
export function refillEpisodes(
	cache: Map<number, KitsuEpisode[]>,
	page: number,
	filled: KitsuEpisode[],
	shown: KitsuEpisode[] | null
): void {
	const cached = cache.get(page);
	const stills = new Map(
		filled.filter((ep) => ep.thumbnail?.original).map((ep) => [ep.id, ep.thumbnail])
	);
	if (!cached?.some((ep) => filled.some((f) => f.id === ep.id))) return;
	for (const ep of [...cached, ...(shown ?? [])]) {
		const still = stills.get(ep.id);
		if (still && !ep.thumbnail?.original) ep.thumbnail = { ...still };
	}
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
