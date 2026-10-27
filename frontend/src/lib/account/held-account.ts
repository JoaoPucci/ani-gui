// Which tracker account a held write was made for, and whether it is
// still that account. Split from `busy-save.ts` so each file stays
// inside the CRAP gate's per-file bar.

import type { Provider } from './types';
import { accountStore } from './store.svelte';

/** The account a fan-out listed a tracker under: its user, or `null`
 *  when the tracker had no account to name. */
export interface ListedAccount {
	user: string | null | undefined;
}

/** The user behind `provider`'s account — connected, or marked expired
 *  or failed with its account still known — or `null` with no account,
 *  or while the account is being changed. */
function accountUser(provider: Provider): string | null | undefined {
	if (accountStore.accountChanging[provider]) return null;
	const state = accountStore.byProvider[provider];
	return state && 'account' in state && state.account ? state.account.user_id : null;
}

/** The account each of `providers` is listed under, read at once — the
 *  moment a fan-out lists them, before anything it awaits. */
export function listAccounts(providers: readonly Provider[]): Record<string, ListedAccount> {
	return Object.fromEntries(providers.map((p) => [p, { user: accountUser(p) }]));
}

/**
 * A check that a write to `provider` is still for the account it was
 * made for — `listed`, as its fan-out read it, or the account the
 * tracker has now: the same user, not disconnected, switched or being
 * changed. A token refresh, a reconnect or an expiry mark for the same
 * user keeps it. A write listed under no account has none to be for,
 * and the check never passes. A write with no listing and no account
 * now has none to follow, and the check always passes: no production
 * caller writes that way — every fan-out passes its listing — and only
 * tests that drive the ledger directly do.
 */
export function followAccount(provider: Provider, listed?: ListedAccount): () => boolean {
	const account = listed ? listed.user : accountUser(provider);
	if (account === null) {
		const ungated = !listed && !accountStore.accountChanging[provider];
		return () => ungated;
	}
	return () => accountUser(provider) === account;
}

/** Where `provider`'s account stands against the user a write `listed`
 *  it under: still that user (`same`), another user connected in its
 *  place (`other`), or no account to compare — disconnected, being
 *  changed, or listed under none (`gone`). */
export function accountStanding(
	provider: Provider,
	listed: ListedAccount
): 'same' | 'other' | 'gone' {
	return standingOf(listed.user, accountUser(provider));
}

/** Where a tracker's user `now` stands against the user a write was
 *  `listed` under: `gone` when either is unknown, else `same` or
 *  `other`. */
export function standingOf(
	listed: string | null | undefined,
	now: string | null | undefined
): 'same' | 'other' | 'gone' {
	if (listed === null || listed === undefined || now === null || now === undefined) return 'gone';
	return now === listed ? 'same' : 'other';
}

/** Where a tracker's account is now, as `hasLeft` reads it. */
export interface AccountNow {
	/** An account change is under way. */
	changing: boolean;
	/** A sign-in is under way. */
	connecting: boolean;
	/** The tracker is disconnected. */
	disconnected: boolean;
	/** The user its account names, if it names one. */
	user: string | null | undefined;
}

/**
 * Whether a tracker has for good left the user a write was listed under
 * — so a held write need not wait for its end-of-wait check to know it
 * is dropped. Disconnected, or another user's account, is final; a
 * sign-in or an account change under way is not, since either can end
 * on the same user. A write listed under no user was never that user's.
 */
export function hasLeft(listedUser: string | null | undefined, now: AccountNow): boolean {
	if (listedUser === null || listedUser === undefined) return true;
	if (now.changing || now.connecting) return false;
	return now.disconnected || now.user !== listedUser;
}

/** `hasLeft` for `provider`'s account as the store holds it now; never
 *  for a write with no listing, which follows no account. */
export function accountLeft(provider: Provider, listed?: ListedAccount): boolean {
	if (!listed) return false;
	const state = accountStore.byProvider[provider];
	return hasLeft(listed.user, {
		changing: accountStore.accountChanging[provider],
		connecting: state?.kind === 'connecting',
		disconnected: !state || state.kind === 'disconnected',
		user: state && 'account' in state && state.account ? state.account.user_id : null
	});
}
