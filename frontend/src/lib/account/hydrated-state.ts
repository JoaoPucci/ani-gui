/**
 * What `accountStore.hydrate()` makes of each provider's keychain read,
 * and what becomes of the renewal failures it holds. Pure, and split
 * from the store so the store stays inside the CRAP gate's per-file
 * bar.
 */

import type { ReadPersistedAccountResult } from './bridge';
import type { PersistedAccount, Provider, ProviderState } from './types';
import type { RenewalFailure, RenewalFailures } from './renewal-failure';

/** Whether a token's expiry has passed. An unknown expiry (`<= 0`) is
 *  treated as valid. */
export function isExpired(account: PersistedAccount, nowSec: number): boolean {
	if (account.expires_at_epoch_s <= 0) return false;
	return account.expires_at_epoch_s <= nowSec;
}

/** Whether `account` carries the bearer `state` holds. */
export function isSameBearer(state: ProviderState, account: PersistedAccount): boolean {
	return 'account' in state && state.account?.access_token === account.access_token;
}

/**
 * The state a keychain read seeds, given the state held before it. A
 * token past its expiry is expired; so is the bearer already marked
 * expired here, which the tracker refused whatever its date says.
 */
export function hydratedState(
	prev: ProviderState,
	read: ReadPersistedAccountResult,
	nowSec: number
): ProviderState {
	if (read.ok) {
		const refused = prev.kind === 'expired' && isSameBearer(prev, read.account);
		return isExpired(read.account, nowSec) || refused
			? { kind: 'expired', account: read.account }
			: { kind: 'connected', account: read.account, lastSyncedAt: null };
	}
	if (read.kind === 'not_found') return { kind: 'disconnected' };
	// The token file is on disk but the keychain is unreachable
	// (libsecret/Keychain outage, decrypt failure, or a token refused
	// because safeStorage fell back to basic_text). Surface as error
	// with no account; the page's error-no-account branch exposes
	// Disconnect so the user can call clearToken and remove the orphan
	// file before reconnecting. The message includes the underlying
	// kind (encryption_unavailable / decrypt_error / …) for the
	// diagnostics log; the page renders a friendlier copy.
	return {
		kind: 'error',
		account: null,
		// i18n-ignore: diagnostic text; no surface renders an error state's message
		message: `Keychain read failed: ${read.detail}`
	};
}

/** `failures` with one provider's entry set, the same object when it
 *  already holds that value. */
export function withFailure(
	failures: RenewalFailures,
	provider: Provider,
	failure: RenewalFailure | null
): RenewalFailures {
	return failures[provider] === failure ? failures : { ...failures, [provider]: failure };
}

/** The failures that survive a hydrate: a provider whose keychain
 *  handed back a different bearer is a different sign-in, and owes
 *  nothing to the old one's failure. */
export function failuresAfterHydrate(
	prev: Record<Provider, ProviderState>,
	next: Record<Provider, ProviderState>,
	failures: RenewalFailures
): RenewalFailures {
	let out = failures;
	for (const p of Object.keys(next) as Provider[]) {
		const s = next[p];
		if ('account' in s && s.account && !isSameBearer(prev[p], s.account)) {
			out = withFailure(out, p, null);
		}
	}
	return out;
}

/** The failures left when the user asks to retry: every expired
 *  sign-in's is forgotten, since its renewal is pending again. */
export function failuresAfterRetry(
	byProvider: Record<Provider, ProviderState>,
	failures: RenewalFailures
): RenewalFailures {
	let out = failures;
	for (const p of Object.keys(byProvider) as Provider[]) {
		if (byProvider[p].kind === 'expired') out = withFailure(out, p, null);
	}
	return out;
}
