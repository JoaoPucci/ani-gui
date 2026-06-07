/**
 * Pure data-on-state helpers for the /account page. Extracted from
 * `connect-flow.ts` so the imperative flow stays in one file and the
 * pure helpers stay in this one — keeps each file's CCN below the
 * CRAP ratchet ceiling (`coverage-baseline.json#crap.high_risk_le`).
 *
 * Every helper here is a pure data → data function. None of them
 * touch the network, safeStorage, or the renderer DOM.
 */

import type { PersistedAccount, ProviderState } from './types';

/**
 * Extract the bearer from a prior provider state. The backend derives
 * the user_id from the bearer (Codex P1 #3369956138), so the caller
 * only needs the bearer to call dropListCache. Returns null for
 * disconnected / connecting / errored-without-account states.
 */
export function bearerFor(state: ProviderState): string | null {
	return accountFromState(state)?.access_token ?? null;
}

/**
 * Extract the persisted user_id from a prior provider state — used as
 * the fallback identity in the cache DELETE call when the bearer has
 * expired or been revoked (Codex P2 #3369997650). Returns null for
 * disconnected / connecting / errored-without-account states.
 */
export function userIdFor(state: ProviderState): string | null {
	return accountFromState(state)?.user_id ?? null;
}

/** Internal: pick the persisted account out of any state that has one. */
function accountFromState(state: ProviderState): PersistedAccount | null {
	if (state.kind === 'connected' || state.kind === 'expired') return state.account;
	if (state.kind === 'error' && state.account) return state.account;
	return null;
}

/**
 * Pick the provider state the page should restore to after a failed
 * Connect / Reconnect attempt, given the state the user was in before
 * they clicked the button. Codex P2 #3370011851: unconditionally
 * collapsing to `disconnected` after a Reconnect from `expired` (or a
 * Connect from `error` that still carries an account) hides the
 * persisted token and the Disconnect button until the next `hydrate()`
 * restores it. Restore the account-backed state instead so the page
 * keeps offering Reconnect + Disconnect.
 *
 * Rules:
 *
 *  - `expired` with an account → stays `expired` (the bearer didn't
 *    suddenly become valid; the user just failed to refresh it)
 *  - `error` with an account → stays `error` with the same message —
 *    the new attempt failed too; the prior message is at least as
 *    accurate as a generic one
 *  - anything else (`disconnected`, `connecting`, `error` with no
 *    account) → fall through to `disconnected`, matching the prior
 *    behaviour
 */
export function restoreAfterFailedConnect(prev: ProviderState): ProviderState {
	if (prev.kind === 'expired') return prev;
	if (prev.kind === 'error' && prev.account) return prev;
	return { kind: 'disconnected' };
}

/**
 * Map an OAuth-flow error kind into the matching i18n key suffix the
 * page uses to look up the toast message. Centralised so the page
 * and tests agree on the table.
 */
export function connectErrorKey(kind: string): string {
	switch (kind) {
		case 'port_busy':
			return 'port_busy';
		case 'timeout':
			return 'timeout';
		case 'cancelled':
			return 'cancelled';
		case 'oauth_error':
			return 'oauth_error';
		case 'no_bridge':
			return 'no_bridge';
		default:
			return 'unknown';
	}
}
