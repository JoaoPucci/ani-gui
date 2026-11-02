/**
 * Whether a provider's title names a Kitsu entry outright — one of the
 * entry's own titles, up to case, punctuation and how the cour suffix
 * is spelled. The backend's mapping guard asks the same question
 * (`backend/src/commands/cour_same_name.rs`); this is its port for the
 * stored-binding check, so the two layers judge a binding alike.
 *
 * The cour check reads a Kitsu entry's cour off its slug, and a slug
 * without a suffix reads as cour 1 — wrong for every entry whose slug
 * marks a later season some other way: Gintama' is `gintama-2`,
 * Gintama° `gintama-2015-tv`. Kitsu titles those entries "Gintama
 * Season 2" and "Gintama Season 4", the names the provider lists them
 * under, and a title the entry itself carries is no evidence of
 * another cour.
 *
 * The comparison is the whole name, not the number alone: read the
 * number by itself and "Attack on Titan Season 3 Part 2" agrees with
 * the Season 2 entry.
 */
import type { KitsuAnimeRef } from '$lib/api';

/** Every name the entry carries: canonical, localized, abbreviated. */
export function entryNames(ref: KitsuAnimeRef): string[] {
	// A stale cached shape can lack any of them, the canonical title too.
	return [
		ref.canonical_title,
		...Object.values(ref.titles ?? {}),
		...(ref.abbreviated_titles ?? [])
	].filter((t): t is string => typeof t === 'string');
}

/** Whether `providerTitle` is one of `entryTitles`: the same words once
 *  case and punctuation are folded, or the same name before a trailing
 *  cour suffix with the same cour number however the suffix is spelled
 *  ("Season 2", "2nd Season", "Second Season", "Part 2", "Cour 2"). A
 *  title with no words names nothing. */
export function namesTheEntry(providerTitle: string, entryTitles: readonly string[]): boolean {
	const wanted = words(providerTitle);
	if (wanted.length === 0) return false;
	const wantedKey = wanted.join(' ');
	const wantedCour = splitCour(wanted);
	return entryTitles.some((t) => {
		const have = words(t);
		if (have.join(' ') === wantedKey) return true;
		if (!wantedCour) return false;
		const haveCour = splitCour(have);
		return (
			haveCour !== null && haveCour.name === wantedCour.name && haveCour.cour === wantedCour.cour
		);
	});
}

const COUR_KEYWORDS = new Set(['season', 'part', 'cour']);

/** The ten spelled ordinals the cour parsers read. */
const ORDINAL_WORDS = [
	'first',
	'second',
	'third',
	'fourth',
	'fifth',
	'sixth',
	'seventh',
	'eighth',
	'ninth',
	'tenth'
];

/** The title's words: lowercased, split on everything that is not a
 *  letter or a digit. */
function words(title: string): string[] {
	return title
		.toLowerCase()
		.split(/[^\p{Alphabetic}\p{N}]+/u)
		.filter((w) => w.length > 0);
}

/** The name before a trailing cour suffix, and the cour it names —
 *  `[.., season|part|cour, N]`, `[.., Nst|Nnd|Nrd|Nth, keyword]` or
 *  `[.., second, keyword]`. Null without a suffix, or when nothing
 *  precedes it. */
function splitCour(ws: string[]): { name: string; cour: number } | null {
	if (ws.length < 3) return null;
	const [a, b] = ws.slice(-2);
	let cour: number | null = null;
	if (COUR_KEYWORDS.has(a)) cour = /^\d+$/.test(b) ? courNumber(b) : null;
	else if (COUR_KEYWORDS.has(b)) cour = ordinal(a);
	if (cour === null) return null;
	return { name: ws.slice(0, -2).join(' '), cour };
}

/** A cour written as an ordinal: a number with its suffix, or one of
 *  the spelled ordinals. */
function ordinal(word: string): number | null {
	const spelled = ORDINAL_WORDS.indexOf(word);
	if (spelled >= 0) return spelled + 1;
	const m = /^(\d+)(st|nd|rd|th)$/.exec(word);
	return m ? courNumber(m[1]) : null;
}

/** The largest cour the backend reads: it parses a cour as a `u32`. */
const MAX_COUR = 0xffffffff;

/** An ASCII digit run as a cour, as the backend reads it: none past
 *  `u32`, which also keeps two numbers past double precision from
 *  reading as one. */
function courNumber(digits: string): number | null {
	const trimmed = digits.replace(/^0+(?=\d)/, '');
	if (trimmed.length > String(MAX_COUR).length) return null;
	const n = Number(trimmed);
	return n <= MAX_COUR ? n : null;
}
