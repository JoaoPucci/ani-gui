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

/** Whether `show` has art of its own for the hero: AniList's banner,
 *  or any slot of Kitsu's cover. A show with neither would be shown
 *  as its poster, blurred. */
export function hasRealArt(show: HeroArtInput): boolean {
	if (show.anilist_banner) return true;
	const c = show.cover_image;
	return Boolean(c && (c.large || c.original || c.small));
}

/** The hero's shows and the row's, both in trending order. */
export interface TrendingSplit<T> {
	hero: T[];
	row: T[];
}

/**
 * `shows` with each show once: a show listed more than once keeps its
 * first entry, in its first place, and the rest keep their order. Two
 * entries are the same show when they share an id.
 */
export function dropRepeats<T extends { id: string }>(shows: readonly T[]): T[] {
	const seen = new Set<string>();
	return shows.filter((s) => {
		if (seen.has(s.id)) return false;
		seen.add(s.id);
		return true;
	});
}

/**
 * Split `trending` into the hero's rotation and the row below it.
 *
 * A show the list names more than once counts once, at its first
 * entry ({@link dropRepeats}). The hero takes the first `count` shows
 * with art, in trending order. When fewer than `count` have art, the
 * first shows without it fill the rest, after the ones with art. The
 * row lists every other show in trending order, so the shows the hero
 * passed over lead it: no show leaves Home and none appears twice.
 */
export function splitTrending<T extends HeroArtInput & { id: string }>(
	trending: readonly T[],
	count: number = HERO_ROTATION_COUNT
): TrendingSplit<T> {
	const shows = dropRepeats(trending);
	const ranks = shows.map((_, i) => i);
	const withArt = ranks.filter((i) => hasRealArt(shows[i])).slice(0, count);
	const fill = ranks
		.filter((i) => !hasRealArt(shows[i]))
		.slice(0, Math.max(0, count - withArt.length));
	const taken = new Set([...withArt, ...fill]);
	return {
		hero: [...withArt, ...fill].map((i) => shows[i]),
		row: ranks.filter((i) => !taken.has(i)).map((i) => shows[i])
	};
}
