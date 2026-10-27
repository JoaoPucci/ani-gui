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
import { NO_RENEWAL_FAILURES, type RenewalFailure, type RenewalFailures } from './renewal-failure';

export interface ExpiredProvider {
	provider: Provider;
	username: string;
}

/**
 * What a sign-in notice says: the session ran out with no way to renew
 * it (`expired`), the tracker refused to renew it (`refused`), the
 * tracker could not be reached to renew it (`unreachable`), or the
 * renewal failed for a reason the app cannot tell (`unknown`).
 */
export type SignInNotice = 'expired' | 'refused' | 'unreachable' | 'unknown';

export interface SignInNoticeInfo extends ExpiredProvider {
	notice: SignInNotice;
}

/** The notice a provider's sign-in calls for, or none. */
export function signInNotice(
	state: ProviderState,
	_failure: RenewalFailure | null
): SignInNotice | null {
	void _failure;
	return state.kind === 'expired' ? 'expired' : null;
}

/** Every provider whose sign-in calls for a notice, in priority order. */
export function detectSignInNotices(
	byProvider: Record<Provider, ProviderState>,
	failures: RenewalFailures = NO_RENEWAL_FAILURES
): SignInNoticeInfo[] {
	const out: SignInNoticeInfo[] = [];
	for (const provider of PRIORITY) {
		const state = byProvider[provider];
		const notice = signInNotice(state, failures[provider]);
		if (notice !== null && 'account' in state && state.account) {
			out.push({ provider, username: state.account.username, notice });
		}
	}
	return out;
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
 * Tracks per-provider toast ids so the layout can dismiss a sign-in
 * notice the moment the provider recovers (or is disconnected), even
 * when the user fixes the session from /account or the chip popover
 * instead of pressing the toast action. Pinned toasts (`duration:
 * null`) require explicit dismiss; without this tracker the stale
 * warning lingers after recovery. One toast per incident: a provider
 * whose notice is unchanged is not pushed again, and a notice that
 * changes replaces the one on screen.
 */
export class ExpiryToastTracker {
	private byProvider = new Map<Provider, { notice: SignInNotice; toastId: string }>();

	sync(
		state: Record<Provider, ProviderState>,
		deps: ExpirySyncDeps,
		failures: RenewalFailures = NO_RENEWAL_FAILURES
	): void {
		const now = new Map<Provider, SignInNoticeInfo>(
			detectSignInNotices(state, failures).map((e) => [e.provider, e])
		);
		for (const [provider, shown] of this.byProvider) {
			if (now.get(provider)?.notice !== shown.notice) {
				deps.dismiss(shown.toastId);
				this.byProvider.delete(provider);
			}
		}
		for (const [provider, info] of now) {
			if (this.byProvider.has(provider)) continue;
			const toastId = deps.push({ provider: info.provider, username: info.username });
			this.byProvider.set(provider, { notice: info.notice, toastId });
		}
	}
}
