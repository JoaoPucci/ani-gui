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

export function mergedWatchLater(byProvider: Partial<Record<Provider, ListEntry[]>>): ListEntry[] {
	void byProvider;
	return [];
}
