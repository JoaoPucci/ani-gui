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

const PRIORITY: ReadonlyArray<Provider> = ['anilist', 'mal', 'inhouse'];

export function detectExpiredProviders(
	byProvider: Record<Provider, ProviderState>
): ExpiredProvider[] {
	const out: ExpiredProvider[] = [];
	for (const provider of PRIORITY) {
		const state = byProvider[provider];
		if (state.kind === 'expired') {
			out.push({ provider, username: state.account.username });
		}
	}
	return out;
}

export interface ExpirySyncDeps {
	push(info: ExpiredProvider): string;
	dismiss(toastId: string): void;
}

/**
 * Tracks per-provider toast ids so the layout can dismiss the
 * "session expired" warning the moment the user reconnects (or
 * disconnects) the provider — even when they fix the session from
 * /account or the chip popover directly instead of pressing the
 * toast action. Codex P2 #3375219208: pinned toasts (`duration:
 * null`) require explicit dismiss; without this tracker the stale
 * warning lingers after recovery.
 */
export class ExpiryToastTracker {
	private byProvider = new Map<Provider, string>();

	sync(state: Record<Provider, ProviderState>, deps: ExpirySyncDeps): void {
		const expiredNow = new Map<Provider, ExpiredProvider>(
			detectExpiredProviders(state).map((e) => [e.provider, e])
		);
		for (const [provider, toastId] of this.byProvider) {
			if (!expiredNow.has(provider)) {
				deps.dismiss(toastId);
				this.byProvider.delete(provider);
			}
		}
		for (const [provider, info] of expiredNow) {
			if (this.byProvider.has(provider)) continue;
			this.byProvider.set(provider, deps.push(info));
		}
	}
}
