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
 */
const inFlight: Partial<Record<Provider, Promise<string | null>>> = {};

export function freshBearerFor(provider: Provider): Promise<string | null> {
	const state = accountStore.byProvider[provider];
	if (state.kind !== 'connected') return Promise.resolve(bearerFor(state));
	const pending = inFlight[provider];
	if (pending) return pending;
	const p = freshBearer(
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
		delete inFlight[provider];
	});
	inFlight[provider] = p;
	return p;
}

/** Test-only: clear the in-flight refresh map between cases. */
export function __resetInFlightRefreshes(): void {
	for (const key of Object.keys(inFlight) as Provider[]) {
		delete inFlight[key];
	}
}
