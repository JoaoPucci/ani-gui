/**
 * Which Continue rows' matches are only guesses, for the home page's
 * lifetime. A Continue card whose match is a guess opens its play
 * session marked as such (play-origin.ts); a card matched by the id
 * the row recorded, or by a mapping a play stored, does not.
 */

import type { HistoryEntry, KitsuAnimeRef } from '$lib/api';

export interface RowTrust {
	/** The loader's resolveMatch: resolves the row and notes its verdict. */
	resolveMatch: (entry: HistoryEntry) => Promise<KitsuAnimeRef | null>;
	/** Whether the row's current match is a guess. */
	isGuess: (entryId: string) => boolean;
}

export function createRowTrust(
	resolve: (entry: HistoryEntry) => Promise<{ match: KitsuAnimeRef | null; trusted: boolean }>
): RowTrust {
	const guesses = new Set<string>();
	return {
		resolveMatch: async (entry) => {
			const { match, trusted } = await resolve(entry);
			if (match && !trusted) guesses.add(entry.id);
			else guesses.delete(entry.id);
			return match;
		},
		isGuess: (entryId) => guesses.has(entryId)
	};
}
