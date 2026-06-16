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
	/** Monotonic per-provider counter, bumped on every account state
	 *  change (connect / disconnect / expire / error) and synchronously
	 *  at the start of an async disconnect. Captured before the refresh
	 *  await and re-checked after: if it moved, a disconnect or re-auth
	 *  raced the refresh and the write is superseded — even when the
	 *  store's account snapshot hasn't caught up yet (Codex P2
	 *  #3416616176, #3416668470). */
	generation(provider: Provider): number;
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
	const generationAtStart = deps.generation(provider);
	let tokens;
	try {
		tokens = await deps.refreshTokens(provider, account.refresh_token);
	} catch {
		return { kind: 'failed' };
	}
	// The await yielded — a disconnect or re-auth may have raced the
	// refresh. If the provider's generation moved, that change wins:
	// don't persist the refreshed-from snapshot back (which would
	// resurrect a removed token or clobber a newer session). The counter
	// catches a disconnect even during the window before its async clear
	// updates the store (Codex P2 #3416616176, #3416668470).
	if (deps.generation(provider) !== generationAtStart) {
		return { kind: 'superseded' };
	}
	const refreshed: PersistedAccount = {
		...account,
		access_token: tokens.access_token,
		refresh_token: tokens.refresh_token ?? account.refresh_token,
		expires_at_epoch_s: tokens.expires_at_epoch_s
	};
	// persistAccount routes through the token-write queue, which preserves
	// a rejected write for the caller (e.g. a safeStorage/IPC error). Treat
	// a rejection the same as a resolved {ok:false}: a failed refresh, not
	// an exception that escapes to abort the caller's best-effort flow
	// (Codex P2 #3421439995).
	let persisted;
	try {
		persisted = await deps.persistAccount(provider, refreshed);
	} catch {
		return { kind: 'failed' };
	}
	if (!persisted.ok) return { kind: 'failed' };
	// Re-check after the persist await too: a disconnect / re-auth could
	// have landed in the gap between the network-await check and the
	// safeStorage write completing. If the generation moved, don't report
	// 'refreshed' — the caller must not reconnect the superseded account
	// (Codex P2 #3416732381).
	if (deps.generation(provider) !== generationAtStart) {
		return { kind: 'superseded' };
	}
	return { kind: 'refreshed', account: refreshed };
}

/**
 * Seconds before a token's expiry at which a still-valid token is
 * proactively refreshed, so a long-lived session never hands a
 * just-expired bearer to a write-back or list refresh (Codex P2
 * #3416883107). MAL access tokens last ~1h; a two-minute skew refreshes
 * comfortably ahead of the boundary without churning on every call.
 */
export const REFRESH_SKEW_SECONDS = 120;

/**
 * True when `account` carries a refresh token and its access token is
 * within `REFRESH_SKEW_SECONDS` of expiry (or already past it). A null
 * refresh token (AniList) or unknown expiry (`<= 0`) returns false —
 * neither can be silently refreshed here.
 */
export function needsProactiveRefresh(account: PersistedAccount, nowSec: number): boolean {
	if (!account.refresh_token) return false;
	if (account.expires_at_epoch_s <= 0) return false;
	return account.expires_at_epoch_s - nowSec <= REFRESH_SKEW_SECONDS;
}

export interface FreshBearerDeps extends RefreshFlowDeps {
	/** Commit the refreshed account back to the store (→ `setConnected`). */
	onRefreshed(provider: Provider, account: PersistedAccount): void;
	/** Current wall-clock in epoch ms. Injected for testability. */
	now(): number;
}

/**
 * Return a bearer for a *connected* provider that's safe to send on the
 * next API call, refreshing it first when it's within the skew window
 * of expiry. On a successful refresh the rotated account is committed
 * via `onRefreshed` and the fresh access token is returned. Any failure
 * (refresh threw, persist failed, or a disconnect/re-auth superseded
 * the refresh) falls back to the current bearer — best-effort, the
 * caller's request may still 401 exactly as it did before, but a
 * refreshable token is never left to rot for the life of the session
 * (Codex P2 #3416883107).
 */
export async function freshBearer(
	deps: FreshBearerDeps,
	provider: Provider,
	account: PersistedAccount
): Promise<string> {
	const nowSec = Math.floor(deps.now() / 1000);
	if (!needsProactiveRefresh(account, nowSec)) return account.access_token;
	const outcome = await refreshAccount(deps, provider, account);
	if (outcome.kind === 'refreshed') {
		deps.onRefreshed(provider, outcome.account);
		return outcome.account.access_token;
	}
	return account.access_token;
}

export interface RefreshExpiredDeps extends RefreshFlowDeps {
	/** Snapshot of the store's per-provider state. */
	byProvider(): Record<Provider, ProviderState>;
	/** Called for each provider whose refresh succeeded. */
	onRefreshed(provider: Provider, account: PersistedAccount): void;
	/** True while an account change (e.g. disconnect) is in progress for
	 *  the provider — such a provider is skipped, mirroring the gate in
	 *  `freshBearer` (Codex P2 #3421609159). */
	changing(provider: Provider): boolean;
}

/**
 * Refresh every expired-but-refreshable provider concurrently, marking
 * the successful ones connected via `onRefreshed`. Failures and
 * unrefreshable providers are left as-is (still expired). A provider
 * whose account change is in progress is skipped: a disconnect could
 * have bumped the generation and queued its `clearToken` already, so
 * starting a refresh risks the queued persist landing after the clear
 * and resurrecting the removed token (Codex P2 #3421609159).
 */
export async function refreshExpiredAccounts(deps: RefreshExpiredDeps): Promise<void> {
	const snapshot = deps.byProvider();
	await Promise.all(
		expiredRefreshable(snapshot).map(async (provider) => {
			if (deps.changing(provider)) return;
			// expiredRefreshable already guaranteed the `expired` kind (and a
			// refresh token) on this same snapshot, so the narrow is safe.
			const { account } = snapshot[provider] as Extract<ProviderState, { kind: 'expired' }>;
			const outcome = await refreshAccount(deps, provider, account);
			if (outcome.kind === 'refreshed') deps.onRefreshed(provider, outcome.account);
		})
	);
}
