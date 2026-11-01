/**
 * The shows a guessed history row can be linked to, and what to warn
 * about before linking it to one. Not wired yet.
 */

import type { HistoryEntry, KitsuAnimeRef } from '$lib/api';

/** A show offered for the row, and whether the card shows it now. */
export interface LinkCandidate {
	ref: KitsuAnimeRef;
	current: boolean;
}

/** What a link to a show would be worth a second look for. */
export type LinkWarning = 'season' | 'episode';

/** The Kitsu-searchable words of a history row's show id. */
export function slugSearchTerm(showId: string): string | null {
	void showId;
	return null;
}

/** The candidates, the current show first. */
export function mergeCandidates(
	current: KitsuAnimeRef | null,
	titleHits: KitsuAnimeRef[],
	slugHits: KitsuAnimeRef[]
): LinkCandidate[] {
	void current;
	void titleHits;
	void slugHits;
	return [];
}

/** What to point out before linking `entry` to `ref`. */
export function linkWarnings(entry: HistoryEntry, ref: KitsuAnimeRef): LinkWarning[] {
	void entry;
	void ref;
	return [];
}
