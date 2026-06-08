/**
 * Watch Later rail loader (plan §6.6).
 *
 * Orchestrates the three async steps the home rail needs:
 *   1. Fetch each connected provider's cached Plan-to-Watch via
 *      `fetchCachedList` — survives offline / 5xx via the
 *      internal-secret-gated fallback shipped in PR #60.
 *   2. Merge across providers via `mergedWatchLater` — AniList
 *      first, mal_id-deduped.
 *   3. Batch-bridge the merged mal_ids to Kitsu refs via
 *      `kitsuByMalIds` so the rail renders with the same metadata
 *      and availability filter the rest of the home page uses.
 *
 * Pure dependency-injected for unit-testability — the home page
 * imports default-bound versions; tests substitute stubs.
 */

import type { ListEntry, Provider } from './types';
import type { KitsuAnimeRef } from '$lib/api';
import { mergedWatchLater } from './watch-later';

export interface WatchLaterDeps {
	/** Bearer + fallback user_id per connected provider. Disconnected
	 *  providers must NOT appear in the map; an entry signals "this
	 *  provider should contribute rows to the merge". */
	credentials: Partial<Record<Provider, { bearer: string; userId: string }>>;
	/** $lib/account/api.fetchCachedList — injected so tests can stub
	 *  the network without mocking the global fetch. */
	fetchCachedList: (
		provider: Provider,
		bearer: string,
		fallbackUserId?: string
	) => Promise<ListEntry[]>;
	/** $lib/api.kitsuByMalIds — same rationale. */
	kitsuByMalIds: (malIds: number[]) => Promise<KitsuAnimeRef[]>;
}

/**
 * Run the rail loader end-to-end. Returns the Kitsu refs in
 * merge order. Empty input (no connected provider) → empty output.
 * Cache misses on individual providers are swallowed so one
 * provider being unreachable doesn't blank the entire rail.
 */
export async function loadWatchLater(deps: WatchLaterDeps): Promise<KitsuAnimeRef[]> {
	const providers = Object.keys(deps.credentials) as Provider[];
	if (providers.length === 0) return [];

	const byProvider: Partial<Record<Provider, ListEntry[]>> = {};
	await Promise.all(
		providers.map(async (provider) => {
			const cred = deps.credentials[provider];
			if (!cred) return;
			try {
				byProvider[provider] = await deps.fetchCachedList(provider, cred.bearer, cred.userId);
			} catch {
				/* Per-provider failure is non-fatal: leave the entry empty
				   so the merge proceeds with whatever else succeeded. */
			}
		})
	);

	const merged = mergedWatchLater(byProvider);
	const malIds = merged.map((e) => e.mal_id).filter((id): id is number => typeof id === 'number');
	if (malIds.length === 0) return [];

	return deps.kitsuByMalIds(malIds);
}
