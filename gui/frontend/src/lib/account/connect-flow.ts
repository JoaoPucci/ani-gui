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
	persistAccount(provider: Provider, payload: PersistedAccount): Promise<boolean>;
}

export type ConnectFlowResult =
	| { kind: 'connected'; account: PersistedAccount }
	| { kind: 'cancelled' }
	| { kind: 'oauth_error'; reason: string }
	| { kind: 'state_mismatch' }
	| { kind: 'persist_failed' }
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
	const ok = await deps.persistAccount(provider, account);
	if (!ok) return { kind: 'persist_failed' };
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
	clearPersistedAccount(provider: Provider): Promise<boolean>;
	dropListCache(provider: Provider, userId: string, bearer: string): Promise<void>;
}

/**
 * Extract `{ user_id, access_token }` from a prior provider state so
 * the caller doesn't have to walk the discriminated union inline.
 * Returns nulls for the disconnected / connecting cases.
 */
export function bearerAndUserIdFor(
	state: ProviderState
): { userId: string; bearer: string } | null {
	if (state.kind === 'connected' || state.kind === 'expired') {
		return {
			userId: state.account.user_id,
			bearer: state.account.access_token
		};
	}
	if (state.kind === 'error' && state.account) {
		return {
			userId: state.account.user_id,
			bearer: state.account.access_token
		};
	}
	return null;
}

/**
 * Disconnect: clear safeStorage first, then drop the cache rows.
 * Cache eviction errors are swallowed because the next resync will
 * overwrite anyway and the disconnect itself is the user-facing
 * action.
 */
export async function disconnectAccount(
	provider: Provider,
	prevState: ProviderState,
	deps: DisconnectFlowDeps
): Promise<void> {
	const ids = bearerAndUserIdFor(prevState);
	await deps.clearPersistedAccount(provider);
	if (ids) {
		try {
			await deps.dropListCache(provider, ids.userId, ids.bearer);
		} catch {
			/* eviction failure non-fatal — next sync overwrites */
		}
	}
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
