/**
 * Shared reactive holder for the user's chosen primary tracker.
 *
 * `config.primary_account` is the persisted source of truth, but the
 * topbar chip (mounted in the layout) and the picker (on /account)
 * live in different component trees. Routing the value through a
 * module-level rune store lets the picker update the chip + rail
 * instantly on selection instead of only after the next config
 * re-fetch. Pattern mirrors `download/store.svelte.ts` and
 * `account/store.svelte.ts`.
 *
 * Seeded from config wherever config is loaded (layout + /account);
 * `parsePrimaryProvider` does the string→Provider coercion at those
 * call sites so this store only ever holds a `Provider | null`.
 */

import type { Provider } from './types';

class PrimaryAccountStore {
	value = $state<Provider | null>(null);

	set(provider: Provider | null): void {
		this.value = provider;
	}
}

export const primaryAccountStore = new PrimaryAccountStore();
