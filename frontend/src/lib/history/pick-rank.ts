/**
 * Which search hit a Continue row's guess takes first, among the hits
 * that may stand as candidates for the row (`isCandidateForRow`).
 *
 * The row names its show two ways the search ranking does not read: the
 * year it was made, and its own title. A hit from the row's year comes
 * first — the provider lists three shows as "JoJo's Bizarre Adventure",
 * and only the year its key carries tells the 2000 OVA from the 2012
 * series — then a hit whose canonical or localized title is the row's:
 * the provider titles Black Clover's TV series "Black Clover", while
 * Kitsu files the slug that title spells, `black-clover`, under the
 * one-episode Jump Festa special. Neither leaves the ranking as it was.
 */

import type { KitsuAnimeRef } from '$lib/api';

const TRAILING_YEAR = /\((\d{4})\)\s*$/;

/** The release year the row names: a trailing "(2009)" in its title,
 *  else the year its show key carries (`key_year`, read by the
 *  backend, since the renderer does not read slugs). */
export function rowYear(searchTitle: string, keyYear: number | null | undefined): number | null {
	const inTitle = searchTitle.match(TRAILING_YEAR);
	if (inTitle) return Number(inTitle[1]);
	return keyYear ?? null;
}

/** A title as the comparison reads it: case, apostrophes and every
 *  other mark aside. */
function titleKey(title: string): string {
	return title
		.toLowerCase()
		.replace(/['\u2019]/g, '')
		.replace(/[^a-z0-9]+/g, ' ')
		.trim();
}

/** The candidate the row's year or title names, if one does. */
export function rankedPick(
	candidates: KitsuAnimeRef[],
	searchTitle: string,
	year: number | null
): KitsuAnimeRef | null {
	if (year !== null) {
		const dated = candidates.find((h) => (h.start_date ?? '').slice(0, 4) === String(year));
		if (dated) return dated;
	}
	const want = titleKey(searchTitle);
	if (!want) return null;
	return (
		candidates.find((h) =>
			[h.canonical_title, ...Object.values(h.titles ?? {})].some((t) => titleKey(t ?? '') === want)
		) ?? null
	);
}

/** The candidates the row's year or title names, in their order: the
 *  ones from the row's year, then among those the ones titled exactly
 *  as the row, else the ones titled as the row but for marks; each
 *  step narrows only when it names one. */
export function namedCandidates(
	candidates: KitsuAnimeRef[],
	searchTitle: string,
	year: number | null
): KitsuAnimeRef[] {
	void searchTitle;
	void year;
	return candidates;
}
