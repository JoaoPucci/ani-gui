/**
 * Imperative connect / disconnect flow for the /account page,
 * extracted from `+page.svelte` so the page itself stays a thin
 * adapter and the flow logic is unit-testable.
 *
 * Per AGENTS.md §2: imperative logic that grows beyond a handful of
 * lines lives in `$lib` with tests, not inline in `<script>`.
 *
 * Both `connectAccount` and `disconnectAccount` accept all I/O
 * dependencies as a `deps` object so the tests can plug in stubs
 * for buildAuthUrl, openOAuth, exchangeCode, fetchMe, etc.
 */

import type { PersistedAccount, Provider, ProviderState } from './types';
import { bearerFor, userIdFor } from './state-helpers';

// Re-export the data-on-state helpers from their new home so callers
// importing them from connect-flow continue to compile. The helpers
// themselves live in `state-helpers.ts` to keep each file's CCN below
// the CRAP ratchet ceiling.
export { bearerFor, connectErrorKey, restoreAfterFailedConnect, userIdFor } from './state-helpers';

export interface ConnectFlowDeps {
	generateState(): string;
	generatePkce(): { verifier: string; challenge: string; method: 'plain' | 'S256' };
	buildAuthUrl(
		provider: Provider,
		req: { state: string; pkce: { verifier: string; challenge: string; method: string } }
	): Promise<{ url: string }>;
	openOAuth(
		authUrl: string
	): Promise<
		{ ok: true; code: string; state: string } | { ok: false; kind: string; message?: string }
	>;
	exchangeCode(
		provider: Provider,
		req: { code: string; pkce: { verifier: string; challenge: string; method: string } }
	): Promise<{
		access_token: string;
		refresh_token: string | null;
		expires_at_epoch_s: number;
	}>;
	fetchMe(
		provider: Provider,
		bearer: string
	): Promise<{
		user_id: string;
		username: string;
		avatar_url: string | null;
	}>;
	persistAccount(
		provider: Provider,
		payload: PersistedAccount
	): Promise<{ ok: true } | { ok: false; kind: string; detail?: string }>;
}

export type ConnectFlowResult =
	| { kind: 'connected'; account: PersistedAccount }
	| { kind: 'cancelled' }
	| { kind: 'oauth_error'; reason: string }
	| { kind: 'state_mismatch' }
	| { kind: 'persist_failed'; reason?: string }
	| { kind: 'api_error'; status?: number };

/**
 * Run the OAuth flow end-to-end. Returns a discriminated result the
 * caller turns into store mutations + toasts.
 */
export async function connectAccount(
	provider: Provider,
	deps: ConnectFlowDeps
): Promise<ConnectFlowResult> {
	const state = deps.generateState();
	const pkce = deps.generatePkce();
	let auth;
	try {
		auth = await deps.buildAuthUrl(provider, { state, pkce });
	} catch (err) {
		return { kind: 'api_error', status: errStatus(err) };
	}
	const callback = await deps.openOAuth(auth.url);
	if (!callback.ok) {
		return { kind: 'oauth_error', reason: callback.kind };
	}
	if (callback.state !== state) {
		return { kind: 'state_mismatch' };
	}
	let tokens;
	try {
		tokens = await deps.exchangeCode(provider, { code: callback.code, pkce });
	} catch (err) {
		return { kind: 'api_error', status: errStatus(err) };
	}
	let profile;
	try {
		profile = await deps.fetchMe(provider, tokens.access_token);
	} catch (err) {
		return { kind: 'api_error', status: errStatus(err) };
	}
	const account: PersistedAccount = {
		access_token: tokens.access_token,
		refresh_token: tokens.refresh_token,
		expires_at_epoch_s: tokens.expires_at_epoch_s,
		user_id: profile.user_id,
		username: profile.username,
		avatar_url: profile.avatar_url
	};
	const persisted = await deps.persistAccount(provider, account);
	if (!persisted.ok) return { kind: 'persist_failed', reason: persisted.kind };
	return { kind: 'connected', account };
}

function errStatus(err: unknown): number | undefined {
	if (err && typeof err === 'object' && 'status' in err) {
		const s = (err as { status?: unknown }).status;
		if (typeof s === 'number') return s;
	}
	return undefined;
}

export interface DisconnectFlowDeps {
	/**
	 * Synchronously mark this provider as changing BEFORE any async work,
	 * so an in-flight boot token refresh is superseded and can't
	 * re-persist / reconnect the account mid-disconnect. Required of every
	 * caller (the /account page and the topbar chip) so no disconnect path
	 * can forget it (Codex P2 #3416668470, #3416762784).
	 */
	beginAccountChange(): void;
	clearPersistedAccount(provider: Provider): Promise<boolean>;
	dropListCache(provider: Provider, bearer: string, fallbackUserId?: string): Promise<void>;
	/**
	 * Codex P2 #3371658227: orphan-token disconnect path. When the
	 * prior state had no decoded account (hydrate found the file
	 * unreadable), there's no bearer to send to dropListCache —
	 * provider-wide clear is the only path that can drop the rows.
	 */
	dropProviderCache(provider: Provider): Promise<void>;
}

/**
 * Disconnect: drop the cache rows BEFORE clearing safeStorage (the
 * cache delete still needs a live bearer to send), then clear the
 * local tokens. Cache-eviction errors stay swallowed — best-effort —
 * but a token-clear failure is fatal: per Codex P2 #3369988183, if
 * `clearPersistedAccount` returns false the bearer is still on disk
 * and `hydrate()` will restore the account on next launch. Telling
 * the user they're disconnected in that state is a lie, so surface
 * the failure to the caller.
 *
 * Order changed from clear-then-drop (PR #1 v1) to drop-then-clear
 * because the cache DELETE now requires the bearer; running it after
 * the safeStorage purge means the renderer has already forgotten it.
 */
export type DisconnectResult = { kind: 'ok' } | { kind: 'token_clear_failed' };

export async function disconnectAccount(
	provider: Provider,
	prevState: ProviderState,
	deps: DisconnectFlowDeps
): Promise<DisconnectResult> {
	const bearer = bearerFor(prevState);
	const fallbackUserId = userIdFor(prevState) ?? undefined;
	if (bearer) {
		try {
			// Pass the safeStorage-persisted user_id as fallback so the
			// backend can still clear the cache when the bearer has
			// expired (Codex P2 #3369997650).
			await deps.dropListCache(provider, bearer, fallbackUserId);
		} catch {
			/* eviction failure non-fatal — next sync overwrites */
		}
	} else {
		// Codex P2 #3371658227: no bearer = orphan-token disconnect
		// (hydrate's unreadable-token error state). The per-user
		// delete needs a user_id we don't have; fall through to the
		// provider-wide clear so PRIVACY.md's "list cache dropped on
		// disconnect" promise still holds. Gated server-side by the
		// renderer-only internal secret.
		try {
			await deps.dropProviderCache(provider);
		} catch {
			/* same best-effort policy — next launch can retry */
		}
	}
	const ok = await deps.clearPersistedAccount(provider);
	if (!ok) return { kind: 'token_clear_failed' };
	return { kind: 'ok' };
}
