/**
 * Which trending shows the home hero rotates through, and which the
 * trending row below it lists: the renderer's side of the backend's
 * pick (backend/src/hero, `POST /api/home/hero`).
 *
 * The pick and its rules are the backend's: a show listed twice counts
 * once, the first three shows with art of their own lead (airing ones
 * first while the `hero_airing_first` flag is on), shows without art
 * fill what is left, and every other show goes to the row in trending
 * order. Home sends the trending shows its availability filter kept,
 * and draws the ids the answer gives as the shows they name.
 */

import {
	heroPick,
	type HeroPick,
	type HeroPickBody,
	type HeroShow,
	type KitsuAnimeRef,
	type ReadOptions
} from '$lib/api';

/** The hero's shows and the row's, as the pick ordered them. */
export interface TrendingSplit<T> {
	hero: T[];
	row: T[];
}

/** Before any pick: nothing in the hero, nothing in the row. */
export const NO_SPLIT: TrendingSplit<never> = Object.freeze({
	hero: [],
	row: []
}) as TrendingSplit<never>;

/** What the pick reads of a trending show: its id, its title for the
 *  walk, its status, and the art the hero could draw. */
export function heroShow(show: KitsuAnimeRef): HeroShow {
	const c = show.cover_image;
	return {
		id: show.id,
		title: show.canonical_title,
		status: show.status,
		anilist_banner: show.anilist_banner ?? null,
		cover_image: c ? { large: c.large, original: c.original, small: c.small } : null
	};
}

/** The shows the pick's ids name, in the pick's order: a show's first
 *  entry where the list repeats it. An id the list does not hold is
 *  left out. */
export function applyHeroPick<T extends { id: string }>(
	trending: readonly T[],
	pick: Pick<HeroPick, 'hero' | 'row'>
): TrendingSplit<T> {
	const byId = new Map<string, T>();
	for (const show of trending) if (!byId.has(show.id)) byId.set(show.id, show);
	const shows = (ids: readonly string[]) =>
		ids.flatMap((id) => {
			const show = byId.get(id);
			return show ? [show] : [];
		});
	return { hero: shows(pick.hero), row: shows(pick.row) };
}

/** Ask the backend for the pick over `trending`, in trending order,
 *  and draw its answer. Rejects when the backend does. */
export async function pickTrending<T extends KitsuAnimeRef>(
	trending: readonly T[],
	opts: ReadOptions = {},
	ask: (body: HeroPickBody, opts: ReadOptions) => Promise<HeroPick> = heroPick
): Promise<TrendingSplit<T>> {
	const pick = await ask({ shows: trending.map(heroShow) }, opts);
	return applyHeroPick(trending, pick);
}
