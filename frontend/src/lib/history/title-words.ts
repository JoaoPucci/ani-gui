// Whether a Kitsu search hit may be a Continue row's show, by the
// words its titles share with the row's. Kitsu answers a title it does
// not carry with its closest words whatever they are — the provider's
// "There Is Also a Hole in the Student Organization!" brings back Here
// is Greenwood, which shares only "is" with it — so a hit sharing too
// few words with the row's title, by the rule below, is refused before
// the episode count, which a row without a count never
// refuses, can take it. Only rows that did not record the show played
// get this far: a row that recorded it is never matched.
//
// The backend's show-id resolve applies the same rule
// (backend/src/commands/kitsu_title_words.rs); both run the vectors in
// tests/fixtures/title-words/vectors.json. The rule:
//
//  - A title's words are its runs of ASCII letters and digits, ASCII
//    letters folded to lowercase, articles (the, a, an) left out. Any
//    other character separates words, so a title in another script
//    has none.
//  - Sequel markers are not words either: season, part and cour (and
//    their plurals), first to tenth, the roman numerals ii to ix but
//    for v, numbers, and ordinals written with digits (2nd). Two shows'
//    second seasons share those and nothing else.
//  - The hit's titles are its canonical title, its localized titles,
//    its abbreviations and its slug's words.
//  - A row title and a hit title share words when the shared words
//    reach a third of both, when either title's words (two or more)
//    all appear in the other, or when the hit title is one word that
//    is the row title's first, five letters or longer.
//  - A hit is refused only when titles were compared and no pair
//    shares words. With nothing to compare, nothing is refused.

import type { KitsuAnimeRef } from '$lib/api';

const ARTICLES = new Set(['the', 'a', 'an']);
const SEQUEL_MARKERS = new Set([
	'season',
	'seasons',
	'part',
	'parts',
	'cour',
	'cours',
	'first',
	'second',
	'third',
	'fourth',
	'fifth',
	'sixth',
	'seventh',
	'eighth',
	'ninth',
	'tenth',
	'ii',
	'iii',
	'iv',
	'vi',
	'vii',
	'viii',
	'ix'
]);
const NUMBER_OR_ORDINAL = /^\d+(?:st|nd|rd|th)?$/;
const LEADING_WORD_MIN = 5;

function isWord(w: string): boolean {
	return w.length > 0 && !ARTICLES.has(w) && !SEQUEL_MARKERS.has(w) && !NUMBER_OR_ORDINAL.test(w);
}

/** A title's words, in order. */
export function titleWords(title: string): string[] {
	return title
		.replace(/[A-Z]/g, (c) => c.toLowerCase())
		.split(/[^a-z0-9]+/)
		.filter(isWord);
}

function hitTitles(hit: KitsuAnimeRef): string[] {
	return [
		hit.canonical_title,
		...Object.values(hit.titles ?? {}),
		...(hit.abbreviated_titles ?? []),
		hit.slug ? hit.slug.replace(/-/g, ' ') : ''
	].filter((t): t is string => !!t);
}

function pairShares(row: Set<string>, hit: Set<string>, lead: string): boolean {
	let shared = 0;
	for (const w of row) if (hit.has(w)) shared++;
	// A third of both, in whole numbers: a share as a fraction rounds.
	if (3 * shared >= row.size && 3 * shared >= hit.size) return true;
	if (shared === row.size && row.size >= 2) return true;
	if (shared === hit.size && hit.size >= 2) return true;
	return hit.size === 1 && lead.length >= LEADING_WORD_MIN && hit.has(lead);
}

/** Whether `hit` may be the show the row's `titles` name. */
export function sharesWords(titles: string[], hit: KitsuAnimeRef): boolean {
	const candidates = hitTitles(hit)
		.map((t) => new Set(titleWords(t)))
		.filter((c) => c.size > 0);
	let compared = false;
	for (const title of titles) {
		const words = titleWords(title);
		if (words.length === 0) continue;
		const row = new Set(words);
		for (const c of candidates) {
			compared = true;
			if (pairShares(row, c, words[0])) return true;
		}
	}
	return !compared;
}
