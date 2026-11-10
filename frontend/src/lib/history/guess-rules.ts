// The hits a Continue row's guess may not take, asked of the backend
// (backend/src/commands/continue_guess.rs) — split from match.ts so its
// sequencing stays readable and inside the complexity ratchet.

import { kitsuGuessRuledOut, type KitsuAnimeRef } from '$lib/api';
import type { ResumeTarget } from './resolve';

/** `hits` less the ones the row rules out: hits past the episode it
 *  reached (for a row a provider the app walks wrote), of another
 *  format than its title names, carrying none of its title's first
 *  word, or of another season than its title names; and, when every
 *  other hit is one, the hits that are another movie than its title
 *  names, a recap neither its title nor its show key names, or of
 *  another season than its show key names
 *  (backend/src/commands/continue_guess.rs). A hit ruled out is never
 *  offered, even when none is left, and the row goes on to the resolve
 *  from its show key's words. A failed call rules none out, so the
 *  guess picks as it did before the rules, and marks `check` failed so
 *  the pick is not stored. */
export async function admitted(
	preliminary: ResumeTarget,
	hits: KitsuAnimeRef[],
	check: { failed: boolean }
): Promise<KitsuAnimeRef[]> {
	if (hits.length === 0) return hits;
	let out: Set<string>;
	try {
		out = new Set(
			await kitsuGuessRuledOut(
				preliminary.searchTitle,
				preliminary.displayEpisode,
				hits,
				preliminary.allmangaShowId
			)
		);
	} catch {
		check.failed = true;
		return hits;
	}
	return hits.filter((h) => !out.has(h.id));
}
