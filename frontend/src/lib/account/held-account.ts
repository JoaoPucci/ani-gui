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
