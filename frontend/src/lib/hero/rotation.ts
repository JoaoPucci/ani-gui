/**
 * Which trending shows the home hero rotates through, and which the
 * trending row below it lists.
 */

/** How many shows the home hero rotates through. */
export const HERO_ROTATION_COUNT = 3;

/** What of a show's artwork decides whether it has art of its own. */
export interface HeroArtInput {
	anilist_banner?: string | null;
	cover_image: { large?: string | null; original?: string | null; small?: string | null } | null;
}

/** Whether `show` has art of its own for the hero. Every show counts
 *  for now: the hero takes the top of the list whatever it carries. */
export function hasRealArt(show: HeroArtInput): boolean {
	void show;
	return true;
}

/** The hero's shows and the row's, both in trending order. */
export interface TrendingSplit<T> {
	hero: T[];
	row: T[];
}

/** Split `trending` into the hero's rotation and the row below it. */
export function splitTrending<T extends HeroArtInput>(
	trending: readonly T[],
	count: number = HERO_ROTATION_COUNT
): TrendingSplit<T> {
	return { hero: trending.slice(0, count), row: trending.slice(count) };
}
