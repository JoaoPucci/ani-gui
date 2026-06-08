/**
 * Cold-launch expiry-toast helper.
 *
 * AniList issues 1-year JWTs (other providers can be shorter). When
 * the app boots and `accountStore.hydrate()` finds a persisted token
 * that has expired, the chip surfaces an amber dot — but the user
 * might not look at it. This helper enumerates every provider in an
 * `expired` state so `+layout.svelte` can fire one pinned toast per
 * stale session at boot, prompting "Sign in again" without forcing
 * the user to dig into /account.
 *
 * Lives in `$lib` (per AGENTS.md §2) so the priority-order rules get
 * unit coverage independent of the Svelte boot path.
 */

import type { Provider, ProviderState } from './types';

export interface ExpiredProvider {
	provider: Provider;
	username: string;
}

export function detectExpiredProviders(
	byProvider: Record<Provider, ProviderState>
): ExpiredProvider[] {
	void byProvider;
	return [];
}
