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
	return [
		ref.canonical_title,
		...Object.values(ref.titles ?? {}),
		...(ref.abbreviated_titles ?? [])
	];
}

/** Whether `providerTitle` is one of `entryTitles`: the same words once
 *  case and punctuation are folded, or the same name before a trailing
 *  cour suffix with the same cour number however the suffix is spelled
 *  ("Season 2", "2nd Season", "Second Season", "Part 2", "Cour 2"). A
 *  title with no words names nothing. */
export function namesTheEntry(providerTitle: string, entryTitles: readonly string[]): boolean {
	// Red stub: the comparison lands with its green commit.
	void providerTitle;
	void entryTitles;
	return false;
}
