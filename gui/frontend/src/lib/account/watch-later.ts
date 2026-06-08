/**
 * Merge the per-provider Plan-to-Watch cache rows into a single
 * deduped list for the home page's Watch Later rail (PR #2, plan
 * §6.6). Pure helper so the rail's data path is unit-testable
 * without any HTTP / store mocking.
 *
 * Order: AniList first (richer metadata via Kitsu's mappings),
 * MAL second. Dedupe key: `mal_id` — the cross-provider bridge id
 * AniList exposes as `idMal` and MAL returns identically. Entries
 * without a `mal_id` can't be deduped but still render (rare
 * AniList-only titles).
 */

import type { ListEntry, Provider } from './types';

/**
 * Per plan §6.6: AniList first (richer metadata via Kitsu's
 * mappings), then MAL. `inhouse` isn't in scope for the rail —
 * it's reserved for the future native provider that doesn't have
 * an off-app library to mirror.
 */
const MERGE_ORDER: ReadonlyArray<Provider> = ['anilist', 'mal'];

export function mergedWatchLater(byProvider: Partial<Record<Provider, ListEntry[]>>): ListEntry[] {
	const seen = new Set<number>();
	const out: ListEntry[] = [];
	for (const provider of MERGE_ORDER) {
		const rows = byProvider[provider];
		if (!rows) continue;
		for (const entry of rows) {
			if (entry.status !== 'planning') continue;
			if (entry.mal_id != null) {
				if (seen.has(entry.mal_id)) continue;
				seen.add(entry.mal_id);
			}
			out.push(entry);
		}
	}
	return out;
}
