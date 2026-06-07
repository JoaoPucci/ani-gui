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

export function fetchAndCacheList(
	provider: Provider,
	userId: string,
	bearer: string
): Promise<ListEntry[]> {
	return postJson<ListEntry[]>(`/api/account/list/${provider}`, { user_id: userId }, bearer);
}

export function fetchCachedList(
	provider: Provider,
	userId: string,
	bearer: string
): Promise<ListEntry[]> {
	// Bearer required even on the cached read — see backend api/account.rs
	// `get_cached_list` rationale (Codex P2 #3369941703). The renderer
	// already has the token in safeStorage from the connect flow.
	const q = new URLSearchParams({ user_id: userId }).toString();
	return getJson<ListEntry[]>(`/api/account/list/${provider}/cached?${q}`, bearer);
}

export function dropListCache(provider: Provider, userId: string, bearer: string): Promise<void> {
	// Same auth gate as fetchCachedList.
	const q = new URLSearchParams({ user_id: userId }).toString();
	return deleteEndpoint(`/api/account/list/${provider}/cache?${q}`, bearer);
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
