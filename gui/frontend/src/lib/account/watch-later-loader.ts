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

/**
 * Mirror of the backend's
 * `commands::account::WATCH_LATER_BRIDGE_MAX_IDS`. Keep in sync —
 * if these drift the loader will fire requests the backend rejects
 * and the rail blanks (Codex P2 #3373907898 caught the original
 * unbounded version doing exactly that for users with 501+ planned
 * titles).
 */
export const WATCH_LATER_BRIDGE_MAX_IDS = 500;

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
	/** User's chosen lead tracker (config.primary_account, coerced).
	 *  Leads the merge so its rows render first and win the mal_id
	 *  dedupe; unset keeps the AniList-first order. */
	primary?: Provider | null;
}

/**
 * Run the rail loader end-to-end. Returns the Kitsu refs in
 * merge order. Empty input (no connected provider) → empty output.
 * A single provider being unreachable is non-fatal — the merge
 * proceeds with whatever else succeeded. But if EVERY connected
 * provider's fetch fails, the loader rejects rather than resolving
 * `[]`: a total failure is distinct from a genuinely empty list, and
 * the caller surfaces a retry state for it instead of "nothing
 * planned" (Codex P2 #3415603155).
 */
export async function loadWatchLater(deps: WatchLaterDeps): Promise<KitsuAnimeRef[]> {
	const providers = Object.keys(deps.credentials) as Provider[];
	if (providers.length === 0) return [];

	const byProvider: Partial<Record<Provider, ListEntry[]>> = {};
	let anySucceeded = false;
	await Promise.all(
		providers.map(async (provider) => {
			const cred = deps.credentials[provider];
			if (!cred) return;
			try {
				byProvider[provider] = await deps.fetchCachedList(provider, cred.bearer, cred.userId);
				anySucceeded = true;
			} catch {
				/* Per-provider failure is non-fatal here: leave the entry
				   empty so the merge proceeds with whatever else succeeded.
				   The all-failed case is handled right after. */
			}
		})
	);

	// Every connected provider failed — that's a load failure, not an
	// empty list. Reject so the caller can show its retry affordance.
	if (!anySucceeded) {
		throw new Error('loadWatchLater: every connected provider failed');
	}

	const merged = mergedWatchLater(byProvider, deps.primary);
	const malIds = merged
		.map((e) => e.mal_id)
		.filter((id): id is number => typeof id === 'number')
		.slice(0, WATCH_LATER_BRIDGE_MAX_IDS);
	if (malIds.length === 0) return [];

	return deps.kitsuByMalIds(malIds);
}
