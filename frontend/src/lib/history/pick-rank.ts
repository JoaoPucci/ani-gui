/**
 * Which search hit a Continue row's guess takes first, among the hits
 * that may stand as candidates for the row (`isCandidateForRow`).
 */

import type { KitsuAnimeRef } from '$lib/api';

/** The release year the row names: a trailing "(2009)" in its title,
 *  else the year its show key carries (`key_year`, read by the
 *  backend, since the renderer does not read slugs). */
export function rowYear(searchTitle: string, keyYear: number | null | undefined): number | null {
	void searchTitle;
	void keyYear;
	return null;
}

/** The candidate the row's year or title names, if one does. */
export function rankedPick(
	candidates: KitsuAnimeRef[],
	searchTitle: string,
	year: number | null
): KitsuAnimeRef | null {
	void candidates;
	void searchTitle;
	void year;
	return null;
}
