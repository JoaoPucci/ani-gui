/**
 * Token-refresh flow for hydrate (Codex P2 #3412673586).
 *
 * MAL access tokens expire after ~1h while their refresh token stays
 * valid for far longer. Without this, `hydrate()` would mark the
 * provider `expired` on the access-token clock, `accountStore.connected`
 * would drop it, and the user would be pushed back through OAuth every
 * hour. Instead, on hydrate we exchange the refresh token for a fresh
 * set and re-persist it via safeStorage so the provider stays connected.
 *
 * The logic lives here (plain, dependency-injected) so it's unit-tested
 * away from the store's reactive singleton — the store method is a thin
 * delegate. AniList tokens carry no refresh token, so they fall through
 * to the existing expired → reauth path untouched.
 */

import type { PersistedAccount, Provider, ProviderState } from './types';

export interface RefreshFlowDeps {
	refreshTokens(
		provider: Provider,
		refreshToken: string
	): Promise<{
		access_token: string;
		refresh_token: string | null;
		expires_at_epoch_s: number;
	}>;
	persistAccount(
		provider: Provider,
		payload: PersistedAccount
	): Promise<{ ok: true } | { ok: false; kind: string; detail?: string }>;
	/** Re-read the provider's currently-persisted account, or null if it
	 *  is no longer connected/expired. Used as a post-await staleness
	 *  guard so a refresh can't resurrect a token the user disconnected
	 *  or clobber a newer session (Codex P2 #3416616176). */
	currentAccount(provider: Provider): PersistedAccount | null;
}

export type RefreshOutcome =
	| { kind: 'refreshed'; account: PersistedAccount }
	| { kind: 'unrefreshable' }
	| { kind: 'failed' }
	| { kind: 'superseded' };

/**
 * Providers whose persisted state is `expired` and that carry a refresh
 * token — the only ones a silent refresh can restore. (AniList's null
 * refresh token excludes it here, so it keeps the reauth path.)
 */
export function expiredRefreshable(byProvider: Record<Provider, ProviderState>): Provider[] {
	return (Object.keys(byProvider) as Provider[]).filter((p) => {
		const s = byProvider[p];
		return s.kind === 'expired' && !!s.account.refresh_token;
	});
}

/**
 * Refresh one expired-but-refreshable account: exchange the refresh
 * token, merge the fresh tokens into the existing account (preserving
 * identity, and keeping the old refresh token if the provider didn't
 * rotate one), then re-persist. Fails closed — any error leaves the
 * caller to keep the account expired.
 */
export async function refreshAccount(
	deps: RefreshFlowDeps,
	provider: Provider,
	account: PersistedAccount
): Promise<RefreshOutcome> {
	if (!account.refresh_token) return { kind: 'unrefreshable' };
	let tokens;
	try {
		tokens = await deps.refreshTokens(provider, account.refresh_token);
	} catch {
		return { kind: 'failed' };
	}
	// The await yielded — the user may have disconnected or re-authed this
	// provider in the meantime. Re-read the current persisted account and
	// bail unless it's still the exact one we refreshed from, so we never
	// resurrect a removed token or overwrite a newer session (Codex P2
	// #3416616176).
	const current = deps.currentAccount(provider);
	if (
		!current ||
		current.refresh_token !== account.refresh_token ||
		current.access_token !== account.access_token
	) {
		return { kind: 'superseded' };
	}
	const refreshed: PersistedAccount = {
		...account,
		access_token: tokens.access_token,
		refresh_token: tokens.refresh_token ?? account.refresh_token,
		expires_at_epoch_s: tokens.expires_at_epoch_s
	};
	const persisted = await deps.persistAccount(provider, refreshed);
	if (!persisted.ok) return { kind: 'failed' };
	return { kind: 'refreshed', account: refreshed };
}

export interface RefreshExpiredDeps extends RefreshFlowDeps {
	/** Snapshot of the store's per-provider state. */
	byProvider(): Record<Provider, ProviderState>;
	/** Called for each provider whose refresh succeeded. */
	onRefreshed(provider: Provider, account: PersistedAccount): void;
}

/**
 * Refresh every expired-but-refreshable provider concurrently, marking
 * the successful ones connected via `onRefreshed`. Failures and
 * unrefreshable providers are left as-is (still expired).
 */
export async function refreshExpiredAccounts(deps: RefreshExpiredDeps): Promise<void> {
	const snapshot = deps.byProvider();
	await Promise.all(
		expiredRefreshable(snapshot).map(async (provider) => {
			// expiredRefreshable already guaranteed the `expired` kind (and a
			// refresh token) on this same snapshot, so the narrow is safe.
			const { account } = snapshot[provider] as Extract<ProviderState, { kind: 'expired' }>;
			const outcome = await refreshAccount(deps, provider, account);
			if (outcome.kind === 'refreshed') deps.onRefreshed(provider, outcome.account);
		})
	);
}
