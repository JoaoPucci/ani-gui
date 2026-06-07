/**
 * Typed wrappers around `/api/account/*` backend routes and Electron's
 * account contextBridge.
 *
 * Kept separate from `lib/api.ts` (which is the long-standing wrapper
 * surface for all other routes) so the account integration stays
 * self-contained and easy to remove if needed.
 */

import type { ListEntry, PkceWire, Provider, Tokens, UserProfile } from './types';

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

/**
 * Read the response body for inclusion in `AccountApiError.detail`.
 * Swallows body-read failures (truncated stream, decode error, etc.)
 * so the rejection still carries the status code. Extracted from the
 * post/get/delete helpers so the error path has one tested home
 * instead of three identical inline `.catch(() => '')` arrows.
 */
export async function readErrorBody(res: Response): Promise<string> {
	try {
		return await res.text();
	} catch {
		return '';
	}
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
		throw new AccountApiError(res.status, await readErrorBody(res));
	}
	return (await res.json()) as T;
}

async function getJson<T>(path: string, bearer?: string): Promise<T> {
	const base = await apiBase();
	const headers: Record<string, string> = {};
	if (bearer) headers.authorization = `Bearer ${bearer}`;
	const res = await fetch(base.replace(/\/+$/, '') + path, { headers });
	if (!res.ok) {
		throw new AccountApiError(res.status, await readErrorBody(res));
	}
	return (await res.json()) as T;
}

async function deleteEndpoint(
	path: string,
	bearer?: string,
	extraHeaders?: Record<string, string>
): Promise<void> {
	const base = await apiBase();
	const headers: Record<string, string> = { ...(extraHeaders ?? {}) };
	if (bearer) headers.authorization = `Bearer ${bearer}`;
	const res = await fetch(base.replace(/\/+$/, '') + path, { method: 'DELETE', headers });
	if (!res.ok) {
		throw new AccountApiError(res.status, await readErrorBody(res));
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
	//
	// Codex P2 #3370011855: the fallback alone is exploitable under
	// permissive CORS — a cross-origin tab can send `Bearer garbage`
	// plus a guessed user_id and wipe another user's cache. Send the
	// per-process renderer-only secret so the backend can require
	// proof we're the Electron renderer before honouring the fallback.
	const path = fallbackUserId
		? `/api/account/list/${provider}/cache?fallback_user_id=${encodeURIComponent(fallbackUserId)}`
		: `/api/account/list/${provider}/cache`;
	return deleteEndpoint(path, bearer, internalSecretHeader());
}

function internalSecretHeader(): Record<string, string> {
	const w = (typeof window !== 'undefined' ? window : undefined) as Window | undefined;
	const secret = w?.aniGui?.internalSecret;
	return secret ? { 'x-ani-gui-internal-secret': secret } : {};
}

// Bridge helpers now live in `./bridge.ts` so api.ts can stay focused
// on the HTTP-call surface (which has its own CCN to manage). Re-
// exported here so existing imports `from './api'` keep compiling.
export {
	cancelOAuth,
	clearPersistedAccount,
	openExternal,
	openOAuth,
	persistAccount,
	readPersistedAccount
} from './bridge';
