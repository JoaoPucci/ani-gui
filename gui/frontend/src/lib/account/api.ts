/**
 * Typed wrappers around `/api/account/*` backend routes and Electron's
 * account contextBridge.
 *
 * Kept separate from `lib/api.ts` (which is the long-standing wrapper
 * surface for all other routes) so the account integration stays
 * self-contained and easy to remove if needed.
 */

import type {
	ListEntry,
	OAuthOpenResult,
	PersistedAccount,
	PkceWire,
	Provider,
	Tokens,
	UserProfile
} from './types';

// Window.aniGui augmentation lives in lib/api.ts to keep one source
// of truth for the bridge shape; TypeScript can't merge contradictory
// declarations across files.

// ─── Wrappers around fetch() to the local backend ───────────────────

async function apiBase(): Promise<string> {
	const w = (typeof window !== 'undefined' ? window : undefined) as Window | undefined;
	const base = w?.aniGui?.apiBase;
	if (base) return base;
	// Fall back to vite env for browser-only dev runs.
	const env =
		typeof import.meta !== 'undefined' ? import.meta.env?.VITE_ANI_GUI_API_BASE : undefined;
	if (typeof env === 'string' && env.length > 0) return env;
	throw new Error('ani-gui apiBase is not configured');
}

async function postJson<T>(path: string, body: unknown, bearer?: string): Promise<T> {
	const base = await apiBase();
	const headers: Record<string, string> = { 'content-type': 'application/json' };
	if (bearer) headers.authorization = `Bearer ${bearer}`;
	const res = await fetch(base.replace(/\/+$/, '') + path, {
		method: 'POST',
		headers,
		body: JSON.stringify(body)
	});
	if (!res.ok) {
		const detail = await res.text().catch(() => '');
		throw new AccountApiError(res.status, detail);
	}
	return (await res.json()) as T;
}

async function getJson<T>(path: string, bearer?: string): Promise<T> {
	const base = await apiBase();
	const headers: Record<string, string> = {};
	if (bearer) headers.authorization = `Bearer ${bearer}`;
	const res = await fetch(base.replace(/\/+$/, '') + path, { headers });
	if (!res.ok) {
		const detail = await res.text().catch(() => '');
		throw new AccountApiError(res.status, detail);
	}
	return (await res.json()) as T;
}

async function deleteEndpoint(path: string, bearer?: string): Promise<void> {
	const base = await apiBase();
	const headers: Record<string, string> = {};
	if (bearer) headers.authorization = `Bearer ${bearer}`;
	const res = await fetch(base.replace(/\/+$/, '') + path, { method: 'DELETE', headers });
	if (!res.ok) {
		const detail = await res.text().catch(() => '');
		throw new AccountApiError(res.status, detail);
	}
}

export class AccountApiError extends Error {
	constructor(
		public readonly status: number,
		public readonly detail: string
	) {
		super(`account api ${status}: ${detail || '(no body)'}`);
	}
}

// ─── Backend routes ─────────────────────────────────────────────────

export interface AuthUrlRequest {
	state: string;
	pkce: PkceWire;
}

export interface AuthUrlResponse {
	url: string;
}

export function buildAuthUrl(provider: Provider, req: AuthUrlRequest): Promise<AuthUrlResponse> {
	return postJson<AuthUrlResponse>(`/api/account/auth-url/${provider}`, req);
}

export interface ExchangeCodeRequest {
	code: string;
	pkce: PkceWire;
}

export function exchangeCode(provider: Provider, req: ExchangeCodeRequest): Promise<Tokens> {
	return postJson<Tokens>(`/api/account/exchange-code/${provider}`, req);
}

export function fetchMe(provider: Provider, bearer: string): Promise<UserProfile> {
	return postJson<UserProfile>(`/api/account/me/${provider}`, {}, bearer);
}

export function fetchAndCacheList(provider: Provider, bearer: string): Promise<ListEntry[]> {
	// No user_id in the body — the backend derives the cache owner
	// from the bearer by calling me() internally. Codex P2 #3369972493:
	// a renderer-supplied user_id could be used to poison another
	// user's local cache under permissive CORS.
	return postJson<ListEntry[]>(`/api/account/list/${provider}`, {}, bearer);
}

export function fetchCachedList(provider: Provider, bearer: string): Promise<ListEntry[]> {
	// No user_id query — the backend derives it from the bearer by
	// calling the provider's me() endpoint. Codex P1 #3369956138: a
	// renderer-supplied user_id under permissive CORS was forgeable;
	// the bearer must be the only identity input.
	return getJson<ListEntry[]>(`/api/account/list/${provider}/cached`, bearer);
}

export function dropListCache(
	provider: Provider,
	bearer: string,
	fallbackUserId?: string
): Promise<void> {
	// Backend derives the cache-owner user_id from a live `me()` call.
	// When the bearer has expired or been revoked (disconnect path,
	// Codex P2 #3369997650), the call 401s; pass the safeStorage-
	// persisted user_id as `?fallback_user_id=` so the backend can
	// still clear the cache instead of leaving orphan rows behind.
	const path = fallbackUserId
		? `/api/account/list/${provider}/cache?fallback_user_id=${encodeURIComponent(fallbackUserId)}`
		: `/api/account/list/${provider}/cache`;
	return deleteEndpoint(path, bearer);
}

// ─── Electron preload helpers ───────────────────────────────────────

export function readPersistedAccount(provider: Provider): PersistedAccount | null {
	const bridge = (typeof window !== 'undefined' ? window : undefined)?.aniGui?.account;
	if (!bridge) return null;
	const r = bridge.getToken(provider);
	if (!r.ok) return null;
	return r.payload;
}

export async function persistAccount(
	provider: Provider,
	payload: PersistedAccount
): Promise<boolean> {
	const bridge = (typeof window !== 'undefined' ? window : undefined)?.aniGui?.account;
	if (!bridge) return false;
	const r = await bridge.setToken(provider, payload);
	return r.ok;
}

export async function clearPersistedAccount(provider: Provider): Promise<boolean> {
	const bridge = (typeof window !== 'undefined' ? window : undefined)?.aniGui?.account;
	if (!bridge) return false;
	const r = await bridge.clearToken(provider);
	return r.ok;
}

export async function openOAuth(authUrl: string): Promise<OAuthOpenResult> {
	const bridge = (typeof window !== 'undefined' ? window : undefined)?.aniGui?.account;
	if (!bridge) {
		return {
			ok: false,
			kind: 'no_bridge',
			message: 'Electron preload is missing the account surface'
		};
	}
	return bridge.openOAuth({ authUrl });
}

export async function cancelOAuth(): Promise<boolean> {
	const bridge = (typeof window !== 'undefined' ? window : undefined)?.aniGui?.account;
	if (!bridge) return false;
	return bridge.cancelOAuth();
}

export async function openExternal(url: string): Promise<boolean> {
	const fn = (typeof window !== 'undefined' ? window : undefined)?.aniGui?.openExternal;
	if (!fn) return false;
	return fn(url);
}
