/**
 * Waiting for a provider's account state to move: a promise per waiter,
 * resolved at the provider's next state change. Used by the hydrate's
 * reads to hold a retry until a change under way has settled.
 */

import type { Provider } from './types';

export class AccountChanges {
	private waiters = new Map<Provider, (() => void)[]>();

	/** Resolves at `provider`'s next change. */
	next(provider: Provider): Promise<void> {
		return new Promise((resolve) => {
			this.waiters.set(provider, [...(this.waiters.get(provider) ?? []), resolve]);
		});
	}

	/** `provider`'s state changed: wake whoever waits on it. */
	notify(provider: Provider): void {
		const waiting = this.waiters.get(provider) ?? [];
		this.waiters.delete(provider);
		for (const wake of waiting) wake();
	}
}
