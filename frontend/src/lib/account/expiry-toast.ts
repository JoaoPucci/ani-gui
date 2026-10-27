/**
 * Sign-in notices: which tracker sign-ins need the user, and what to
 * tell them.
 *
 * When a sign-in has run out or its renewal has failed, the chip
 * surfaces an amber dot — but the user might not look at it. This
 * helper decides, per provider, whether a notice is due and which one
 * (see {@link signInNotice}), so `+layout.svelte` can keep one pinned
 * toast per incident on screen until the sign-in recovers or the user
 * acts on it.
 *
 * Lives in `$lib` (per AGENTS.md §2) so the priority-order rules get
 * unit coverage independent of the Svelte boot path.
 */

import type { Provider, ProviderState } from './types';
import { NO_RENEWAL_FAILURES, type RenewalFailure, type RenewalFailures } from './renewal-failure';

const PRIORITY: ReadonlyArray<Provider> = ['anilist', 'mal', 'inhouse'];

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
const NOTICE_FOR_FAILURE: Record<RenewalFailure, SignInNotice> = {
	rejected: 'refused',
	unreachable: 'unreachable',
	unknown: 'unknown'
};

/**
 * The notice a provider's sign-in calls for, or none. A sign-in past
 * its expiry that can be renewed says nothing until the renewal has
 * answered, since it may succeed in a moment; one that cannot be
 * renewed has expired, which is known. A still-valid sign-in is told
 * only that its renewal was refused: its session will end, while a
 * renewal that merely went unanswered is tried again on the next call.
 */
export function signInNotice(
	state: ProviderState,
	failure: RenewalFailure | null
): SignInNotice | null {
	if (state.kind === 'connected') return failure === 'rejected' ? 'refused' : null;
	if (state.kind !== 'expired') return null;
	if (failure !== null) return NOTICE_FOR_FAILURE[failure];
	return state.account.refresh_token ? null : 'expired';
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
	push(info: SignInNoticeInfo): string;
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
			this.byProvider.set(provider, { notice: info.notice, toastId: deps.push(info) });
		}
	}
}
