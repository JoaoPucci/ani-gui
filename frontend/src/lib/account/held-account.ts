// Which tracker account a held write was made for, and whether it is
// still that account. Split from `busy-save.ts` so each file stays
// inside the CRAP gate's per-file bar.

import type { Provider } from './types';
import { accountStore } from './store.svelte';

/** The user behind `provider`'s account — connected, or marked expired
 *  or failed with its account still known — or `null` with no account,
 *  or while the account is being changed. */
function accountUser(provider: Provider): string | null {
	if (accountStore.accountChanging[provider]) return null;
	const state = accountStore.byProvider[provider];
	return state && 'account' in state && state.account ? state.account.user_id : null;
}

/** Whether `provider`'s account is being changed — a disconnect or a
 *  sign-in under way. */
export function accountChanging(provider: Provider): boolean {
	return accountStore.accountChanging[provider];
}

/**
 * Note the account a write to `provider` is made for, and return a check
 * that it is still that account: the same user, not disconnected,
 * switched or being changed. A token refresh, a reconnect or an expiry
 * mark for the same user keeps it. A write made with no account to name
 * — its caller brought its own bearer — has none to follow, and the
 * check always passes.
 */
export function followAccount(provider: Provider): () => boolean {
	const account = accountUser(provider);
	return () => account === null || accountUser(provider) === account;
}
