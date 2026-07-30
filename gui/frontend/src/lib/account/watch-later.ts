/**
 * Merge the per-provider Plan-to-Watch cache rows into a single
 * deduped list for the home page's Watch Later rail (PR #2, plan
 * §6.6). Pure helper so the rail's data path is unit-testable
 * without any HTTP / store mocking.
 *
 * Order: most recently touched first, so the rail leads with what
 * the user planned last rather than with whatever the provider's
 * pagination happened to hand back. Provider order — AniList first
 * (richer metadata via Kitsu's mappings), MAL second — decides ties
 * and decides which copy of a duplicate survives.
 *
 * Dedupe key: `mal_id` — the cross-provider bridge id AniList
 * exposes as `idMal` and MAL returns identically. Entries without a
 * `mal_id` can't be deduped but still render (rare AniList-only
 * titles).
 */

import type { ListEntry, Provider } from './types';

/**
 * Per plan §6.6: AniList first (richer metadata via Kitsu's
 * mappings), then MAL. `inhouse` isn't in scope for the rail —
 * it's reserved for the future native provider that doesn't have
 * an off-app library to mirror.
 */
const MERGE_ORDER: ReadonlyArray<Provider> = ['anilist', 'mal'];

/**
 * Build the merge walk order. When `primary` is one of the rail
 * providers it leads, so its rows win the mal_id dedupe and come
 * first among entries sharing a timestamp; the remaining providers
 * keep their fixed relative order. An unset / non-rail primary
 * leaves `MERGE_ORDER` untouched.
 */
function walkOrder(primary?: Provider | null): ReadonlyArray<Provider> {
	if (!primary || !MERGE_ORDER.includes(primary)) return MERGE_ORDER;
	return [primary, ...MERGE_ORDER.filter((p) => p !== primary)];
}

/**
 * Most recently touched first.
 *
 * Runs on the ALREADY-deduped list, and the order matters: sorting
 * before the dedupe would let the more recently touched copy of a
 * cross-provider duplicate win, quietly replacing the
 * primary-provider rule above with a recency one.
 *
 * `Array.sort` is stable, so entries the providers touched at the
 * same moment keep the walk order — provider order survives as the
 * tie-break rather than as the rule.
 *
 * A row the provider never timestamped arrives as 0 (MAL's parser
 * substitutes it for a missing `updated_at`), which descending puts
 * at the end. That is where something with no known recency belongs;
 * ascending would open the rail with it.
 */
function byRecency(entries: ListEntry[]): ListEntry[] {
	return entries.sort((a, b) => b.updated_at_epoch_s - a.updated_at_epoch_s);
}

export function mergedWatchLater(
	byProvider: Partial<Record<Provider, ListEntry[]>>,
	primary?: Provider | null
): ListEntry[] {
	const seen = new Set<number>();
	const out: ListEntry[] = [];
	for (const provider of walkOrder(primary)) {
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
	return byRecency(out);
}
