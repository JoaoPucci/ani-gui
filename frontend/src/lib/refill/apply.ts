/**
 * What a page does with a read fetched once more after its refill
 * landed. No refill lands yet, so nothing changes.
 */
import type { KitsuAnimeRef, KitsuEpisode } from '$lib/api';

/** Put a refilled episode page in `cache` and into the episodes shown. */
export function refillEpisodes(
	cache: Map<number, KitsuEpisode[]>,
	page: number,
	filled: KitsuEpisode[],
	shown: KitsuEpisode[] | null
): KitsuEpisode[] | null {
	void cache;
	void page;
	void filled;
	return shown;
}

/** The images a refilled detail brings: its cover and AniList banner. */
export interface RefilledImages {
	cover_image: KitsuAnimeRef['cover_image'];
	anilist_banner: KitsuAnimeRef['anilist_banner'];
}

/** The images to put on `current` from its refilled detail. */
export function refilledImages(
	current: KitsuAnimeRef | null,
	fresh: KitsuAnimeRef
): RefilledImages | null {
	void current;
	void fresh;
	return null;
}
