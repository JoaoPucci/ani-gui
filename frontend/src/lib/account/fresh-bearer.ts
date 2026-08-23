/**
 * Live-store bearer resolver for the write-back fan-out and the
 * authoritative Watch Later tracker pull.
 *
 * A connected token within the proactive-refresh skew of expiry is
 * refreshed (re-persisted + the store flipped to the rotated tokens)
 * before it's handed out, so a session left open past MAL's ~1h
 * access-token lifetime never sends a just-expired bearer (Codex P2
 * #3416883107). Non-connected states return whatever bearer they carry
 * (or null), unchanged.
 *
 * Kept as a free function reading `accountStore` directly — mirroring
 * `push-watched`'s `syncWatchedToTrackers` — rather than a store method,
 * so the per-call dependency closures don't push `store.svelte.ts` over
 * the CRAP ratchet's per-file ccn ceiling. The refresh decision +
 * orchestration itself is the unit-tested `freshBearer` in
 * `refresh-flow.ts`.
 */

import type { Provider } from './types';
import { accountStore } from './store.svelte';
import { freshBearer } from './refresh-flow';
import { persistAccount, refreshTokens } from './api';
import { bearerFor } from './state-helpers';

/**
 * Per-provider in-flight refresh promise. When two call sites hit the
 * same near-expiry connected provider at once, both would otherwise
 * reach `freshBearer` with the same snapshot and start independent
 * refresh-token exchanges; with rotating refresh tokens that races two
 * rotations to disk while only one survives in memory (Codex P2
 * #3420173434). Sharing the promise means concurrent callers get the
 * same rotated account from a single exchange. Cleared once it settles
 * so a later call (e.g. after the token drifts back toward expiry)
 * starts a fresh one.
 *
 * The entry is tagged with the `accountGeneration` captured when it
 * started. A caller only reuses it while that generation still matches:
 * if the user disconnected / reconnected the provider mid-flight the
 * generation has advanced, the pending refresh belongs to the old
 * account (and will resolve `superseded`), and reusing it would fall
 * back to the previous session's bearer for the new connection — i.e.
 * the prior user's token (Codex P2 #3420249568). A generation-changed
 * caller starts its own refresh instead.
 */
const inFlight: Partial<Record<Provider, { generation: number; promise: Promise<string | null> }>> =
	{};

export function freshBearerFor(provider: Provider): Promise<string | null> {
	const state = accountStore.byProvider[provider];
	if (state.kind !== 'connected') return Promise.resolve(bearerFor(state));
	// A disconnect/account-change is mid-flight: beginAccountChange ran but
	// byProvider is still `connected` until the async clear finishes. Don't
	// start a refresh — its persist could land after the disconnect's clear
	// in the per-provider FIFO and resurrect the removed token (Codex P2
	// #3421338541). Hand back the current bearer; no rotation, no write.
	if (accountStore.accountChanging[provider]) return Promise.resolve(state.account.access_token);
	const generation = accountStore.accountGeneration[provider];
	const pending = inFlight[provider];
	if (pending && pending.generation === generation) return pending.promise;
	const promise = freshBearer(
		{
			refreshTokens,
			persistAccount,
			generation: (prov) => accountStore.accountGeneration[prov],
			onRefreshed: (prov, account) => accountStore.setConnected(prov, account),
			now: () => Date.now()
		},
		provider,
		state.account
	).finally(() => {
		// Only clear if we're still the active entry — a generation-changed
		// caller may have already replaced us with its own refresh.
		if (inFlight[provider]?.promise === promise) delete inFlight[provider];
	});
	inFlight[provider] = { generation, promise };
	return promise;
}

/** Test-only: clear the in-flight refresh map between cases. */
export function __resetInFlightRefreshes(): void {
	for (const key of Object.keys(inFlight) as Provider[]) {
		delete inFlight[key];
	}
}
