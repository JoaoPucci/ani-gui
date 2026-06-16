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

export function freshBearerFor(provider: Provider): Promise<string | null> {
	const state = accountStore.byProvider[provider];
	if (state.kind !== 'connected') return Promise.resolve(bearerFor(state));
	return freshBearer(
		{
			refreshTokens,
			persistAccount,
			generation: (p) => accountStore.accountGeneration[p],
			onRefreshed: (p, account) => accountStore.setConnected(p, account),
			now: () => Date.now()
		},
		provider,
		state.account
	);
}
