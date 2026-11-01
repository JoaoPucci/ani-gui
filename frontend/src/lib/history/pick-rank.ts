/**
 * Which search hits a Continue row's guess picks among, of the hits
 * that may stand as candidates for the row (`isCandidateForRow`).
 *
 * The row names its show two ways the search ranking does not read: the
 * year it was made, and its own title. The hits from the row's year
 * come first — the provider lists three shows as "JoJo's Bizarre
 * Adventure", and only the year its key carries tells the 2000 OVA from
 * the 2012 series — and among them the hits whose canonical or
 * localized title is the row's: the provider titles Black Clover's TV
 * series "Black Clover", while Kitsu files the slug that title spells,
 * `black-clover`, under the one-episode Jump Festa special. A title
 * matches exactly, case aside, before it matches but for its marks,
 * since Kitsu tells Gintama, Gintama' and Gintama. apart by a mark
 * alone. Each step narrows only when it names a hit, and what it
 * leaves goes on to the rules after (the slug, the cour, Kitsu's own
 * order): two seasons from one year are told apart there.
 */

import type { KitsuAnimeRef } from '$lib/api';

const TRAILING_YEAR = /\((\d{4})\)\s*$/;

/** The years a row can name, as the backend's `key_year` reads them. */
const FIRST_YEAR = 1960;
const LAST_YEAR = 2039;

/** The release year the row names: a trailing "(2009)" in its title,
 *  else the year its show key carries (`key_year`, read by the
 *  backend, since the renderer does not read slugs). */
export function rowYear(searchTitle: string, keyYear: number | null | undefined): number | null {
	const inTitle = Number(searchTitle.match(TRAILING_YEAR)?.[1]);
	if (inTitle >= FIRST_YEAR && inTitle <= LAST_YEAR) return inTitle;
	return keyYear ?? null;
}

/** A title as it is written, case and the apostrophe's glyph aside. */
function exactKey(title: string): string {
	return title
		.trim()
		.toLowerCase()
		.replace(/\u2019/g, "'");
}

/** A title but for its marks: case, apostrophes and every other mark
 *  aside. */
function foldedKey(title: string): string {
	return title
		.toLowerCase()
		.replace(/['\u2019]/g, '')
		.replace(/[^a-z0-9]+/g, ' ')
		.trim();
}

function titlesOf(hit: KitsuAnimeRef): string[] {
	return [hit.canonical_title, ...Object.values(hit.titles ?? {})].map((t) => t ?? '');
}

/** `pool` narrowed to the hits one of whose titles reads as `want`
 *  under `key`, or `pool` itself when none does. */
function titledAs(
	pool: KitsuAnimeRef[],
	want: string,
	key: (title: string) => string
): KitsuAnimeRef[] | null {
	if (!want) return null;
	const named = pool.filter((h) => titlesOf(h).some((t) => key(t) === want));
	return named.length > 0 ? named : null;
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
	let pool = candidates;
	// The year the caller read for the row, else the one its title names.
	const named = year ?? rowYear(searchTitle, null);
	if (named !== null) {
		const dated = pool.filter((h) => (h.start_date ?? '').slice(0, 4) === String(named));
		if (dated.length > 0) pool = dated;
	}
	return (
		titledAs(pool, exactKey(searchTitle), exactKey) ??
		titledAs(pool, foldedKey(searchTitle), foldedKey) ??
		pool
	);
}
