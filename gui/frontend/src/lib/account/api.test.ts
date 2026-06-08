/**
 * Tests for the account HTTP + bridge wrappers. Backend routes are
 * mocked with `vi.spyOn(global, 'fetch')`; the Electron bridge is
 * stubbed via a fake `window.aniGui.account`.
 */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
	AccountApiError,
	buildAuthUrl,
	cancelOAuth,
	clearPersistedAccount,
	dropListCache,
	dropProviderCache,
	exchangeCode,
	fetchAndCacheList,
	fetchCachedList,
	fetchMe,
	openExternal,
	openOAuth,
	persistAccount,
	readErrorBody,
	readPersistedAccount
} from './api';
import type { PersistedAccount, PkceWire } from './types';

const PKCE: PkceWire = { verifier: 'v', challenge: 'c', method: 'plain' };

beforeEach(() => {
	(globalThis as { window?: unknown }).window = {
		aniGui: { apiBase: 'http://127.0.0.1:42337' }
	};
});

afterEach(() => {
	(globalThis as { window?: unknown }).window = undefined;
	vi.restoreAllMocks();
});

function mockFetchJson<T>(body: T, status = 200) {
	return vi.spyOn(global, 'fetch').mockResolvedValue(
		new Response(JSON.stringify(body), {
			status,
			headers: { 'content-type': 'application/json' }
		})
	);
}

describe('readErrorBody', () => {
	it('returns the body text on success', async () => {
		const res = new Response('boom', { status: 500 });
		await expect(readErrorBody(res)).resolves.toBe('boom');
	});

	it('swallows .text() rejection and returns empty string', async () => {
		// A truncated response stream or decode error must not mask
		// the underlying HTTP status — the caller still throws an
		// AccountApiError, just without a body excerpt.
		const res = {
			text: vi.fn().mockRejectedValue(new Error('stream broken'))
		} as unknown as Response;
		await expect(readErrorBody(res)).resolves.toBe('');
	});
});

describe('AccountApiError', () => {
	it('includes the status + detail in its message', () => {
		const err = new AccountApiError(403, 'forbidden');
		expect(err.status).toBe(403);
		expect(err.detail).toBe('forbidden');
		expect(err.message).toContain('403');
		expect(err.message).toContain('forbidden');
	});

	it('falls back to `(no body)` when detail is empty', () => {
		const err = new AccountApiError(500, '');
		expect(err.message).toContain('(no body)');
	});
});

describe('buildAuthUrl', () => {
	it('POSTs to /api/account/auth-url/:provider and returns the URL', async () => {
		const spy = mockFetchJson({ url: 'https://anilist.co/authorize?…' });
		const r = await buildAuthUrl('anilist', { state: 's', pkce: PKCE });
		expect(r.url).toContain('anilist.co/authorize');
		const [url, init] = spy.mock.calls[0];
		expect(String(url)).toContain('/api/account/auth-url/anilist');
		expect((init as RequestInit).method).toBe('POST');
	});

	it('throws AccountApiError on non-2xx', async () => {
		mockFetchJson({}, 500);
		await expect(buildAuthUrl('anilist', { state: 's', pkce: PKCE })).rejects.toBeInstanceOf(
			AccountApiError
		);
	});
});

describe('exchangeCode', () => {
	it('POSTs the code + pkce', async () => {
		const spy = mockFetchJson({
			access_token: 'aaa',
			refresh_token: null,
			expires_at_epoch_s: 999
		});
		const r = await exchangeCode('anilist', { code: 'code', pkce: PKCE });
		expect(r.access_token).toBe('aaa');
		const [, init] = spy.mock.calls[0];
		expect(String((init as RequestInit).body)).toContain('"code":"code"');
	});
});

describe('fetchMe', () => {
	it('sends the bearer in the Authorization header', async () => {
		const spy = mockFetchJson({
			provider: 'anilist',
			user_id: '7',
			username: 'p',
			avatar_url: null,
			stats: null
		});
		await fetchMe('anilist', 'tok');
		const [, init] = spy.mock.calls[0];
		const headers = (init as RequestInit).headers as Record<string, string>;
		expect(headers.authorization).toBe('Bearer tok');
	});
});

describe('fetchAndCacheList', () => {
	it('POSTs an empty body + bearer (backend derives the owner from me())', async () => {
		const spy = mockFetchJson([]);
		await fetchAndCacheList('anilist', 'tok');
		const [url, init] = spy.mock.calls[0];
		expect(String(url)).toContain('/api/account/list/anilist');
		// Codex P2 #3369972493: no user_id in the body; backend
		// derives the cache-write owner from the bearer.
		expect(String((init as RequestInit).body)).not.toContain('user_id');
		const headers = (init as RequestInit).headers as Record<string, string>;
		expect(headers.authorization).toBe('Bearer tok');
	});
});

describe('fetchCachedList', () => {
	it('hits the cached endpoint with the bearer (no user_id when no fallback)', async () => {
		const spy = mockFetchJson([]);
		await fetchCachedList('anilist', 'tok');
		const [url, init] = spy.mock.calls[0];
		expect(String(url)).toContain('/api/account/list/anilist/cached');
		expect(String(url)).not.toContain('fallback_user_id');
		const headers = (init as RequestInit).headers as Record<string, string>;
		expect(headers.authorization).toBe('Bearer tok');
	});

	it('appends fallback_user_id + internal-secret header for offline reads (Codex P2 #3372942241)', async () => {
		// Backend gate: the renderer-only secret authenticates us as
		// the Electron preload before the fallback id is trusted.
		(globalThis as { window?: unknown }).window = {
			aniGui: { apiBase: 'http://127.0.0.1:42337', internalSecret: 'cafebabe' }
		};
		const spy = mockFetchJson([]);
		await fetchCachedList('anilist', 'tok', 'u7');
		const [url, init] = spy.mock.calls[0];
		expect(String(url)).toContain('/api/account/list/anilist/cached?fallback_user_id=u7');
		const headers = (init as RequestInit).headers as Record<string, string>;
		expect(headers['x-ani-gui-internal-secret']).toBe('cafebabe');
		expect(headers.authorization).toBe('Bearer tok');
	});

	it('surfaces fetch failure as AccountApiError', async () => {
		mockFetchJson({}, 404);
		await expect(fetchCachedList('anilist', 'tok')).rejects.toBeInstanceOf(AccountApiError);
	});
});

describe('dropListCache', () => {
	it('DELETEs the cache scoped to the user', async () => {
		const spy = mockFetchJson('');
		await dropListCache('anilist', 'tok');
		const [url, init] = spy.mock.calls[0];
		expect(String(url)).toContain('/api/account/list/anilist/cache');
		expect(String(url)).not.toContain('user_id'); // backend derives from bearer (Codex P1)
		expect((init as RequestInit).method).toBe('DELETE');
	});

	it('threads the fallback user_id into the query when provided', async () => {
		// Codex P2 #3369997650: disconnect-after-expiry path. The
		// renderer hands the safeStorage-persisted user_id back as
		// `?fallback_user_id=` so the backend can still clear the
		// cache when me() 401s.
		const spy = mockFetchJson('');
		await dropListCache('anilist', 'tok', 'u7');
		const [url] = spy.mock.calls[0];
		expect(String(url)).toContain('fallback_user_id=u7');
	});

	it('attaches the internal-secret header when present on window.aniGui', async () => {
		// Codex P2 #3370011855: the renderer-only header gates the
		// backend's fallback path so a cross-origin tab can't wipe
		// another user's cache by guessing the user_id.
		(globalThis as { window?: unknown }).window = {
			aniGui: { apiBase: 'http://127.0.0.1:42337', internalSecret: 'deadbeef' }
		};
		const spy = mockFetchJson('');
		await dropListCache('anilist', 'tok', 'u7');
		const [, init] = spy.mock.calls[0];
		const headers = (init as RequestInit).headers as Record<string, string>;
		expect(headers['x-ani-gui-internal-secret']).toBe('deadbeef');
	});

	it('omits the internal-secret header when window.aniGui has no secret', async () => {
		const spy = mockFetchJson('');
		await dropListCache('anilist', 'tok');
		const [, init] = spy.mock.calls[0];
		const headers = (init as RequestInit).headers as Record<string, string>;
		expect(headers['x-ani-gui-internal-secret']).toBeUndefined();
	});

	it('throws AccountApiError on non-2xx', async () => {
		mockFetchJson('boom', 500);
		await expect(dropListCache('anilist', 'tok')).rejects.toBeInstanceOf(AccountApiError);
	});
});

describe('dropProviderCache', () => {
	// Codex P2 #3371658227: provider-wide clear used by the orphan-
	// token disconnect path (no bearer, no user_id). Backend gates on
	// the internal_secret header so a cross-origin tab can't trigger.
	it('DELETEs the provider-wide cache endpoint with the internal-secret header', async () => {
		(globalThis as { window?: unknown }).window = {
			aniGui: { apiBase: 'http://127.0.0.1:42337', internalSecret: 'cafebabe' }
		};
		const spy = mockFetchJson('');
		await dropProviderCache('anilist');
		const [url, init] = spy.mock.calls[0];
		expect(String(url)).toContain('/api/account/list/anilist/cache/all');
		expect((init as RequestInit).method).toBe('DELETE');
		const headers = (init as RequestInit).headers as Record<string, string>;
		expect(headers['x-ani-gui-internal-secret']).toBe('cafebabe');
		// No Authorization header — this path is gated only by the secret.
		expect(headers.authorization).toBeUndefined();
	});

	it('omits the secret header when one isn’t available (browser-only dev)', async () => {
		// In dev without Electron the secret simply isn't on window;
		// backend rejects, surfacing as AccountApiError. Not collapsed
		// to a noop so dev-time misconfigs are visible.
		const spy = mockFetchJson('');
		await dropProviderCache('anilist');
		const [, init] = spy.mock.calls[0];
		const headers = (init as RequestInit).headers as Record<string, string>;
		expect(headers['x-ani-gui-internal-secret']).toBeUndefined();
	});

	it('throws AccountApiError on non-2xx', async () => {
		mockFetchJson('forbidden', 403);
		await expect(dropProviderCache('anilist')).rejects.toBeInstanceOf(AccountApiError);
	});
});

// ─── Electron preload helpers ───────────────────────────────────────

function payload(): PersistedAccount {
	return {
		access_token: 'a',
		refresh_token: null,
		expires_at_epoch_s: 999,
		user_id: '7',
		username: 'p',
		avatar_url: null
	};
}

function stubBridge(impl: Record<string, unknown>) {
	(globalThis as { window?: { aniGui?: unknown } }).window = {
		aniGui: { account: impl, apiBase: 'http://127.0.0.1:42337' }
	};
}

describe('readPersistedAccount', () => {
	it('returns ok+account when the bridge has one', () => {
		stubBridge({ getToken: () => ({ ok: true, payload: payload() }) });
		const r = readPersistedAccount('anilist');
		expect(r.ok).toBe(true);
		if (r.ok) expect(r.account.username).toBe('p');
	});

	it('returns not_found when the bridge says no token on disk', () => {
		stubBridge({ getToken: () => ({ ok: false, kind: 'not_found' }) });
		const r = readPersistedAccount('anilist');
		expect(r).toEqual({ ok: false, kind: 'not_found' });
	});

	it('returns not_found when no bridge is wired (browser-only dev)', () => {
		(globalThis as { window?: unknown }).window = undefined;
		const r = readPersistedAccount('anilist');
		expect(r).toEqual({ ok: false, kind: 'not_found' });
	});

	it('returns unreadable when the bridge surfaces encryption_unavailable', () => {
		// Codex P2 #3371530183: keychain outage → file is on disk but
		// can't be decrypted. Callers must NOT collapse this to "no
		// account" — the orphan file needs an in-app cleanup path.
		stubBridge({ getToken: () => ({ ok: false, kind: 'encryption_unavailable' }) });
		const r = readPersistedAccount('anilist');
		expect(r).toEqual({ ok: false, kind: 'unreadable', detail: 'encryption_unavailable' });
	});

	it('returns unreadable when the bridge surfaces decrypt_error', () => {
		stubBridge({ getToken: () => ({ ok: false, kind: 'decrypt_error' }) });
		const r = readPersistedAccount('anilist');
		expect(r).toEqual({ ok: false, kind: 'unreadable', detail: 'decrypt_error' });
	});
});

describe('persistAccount', () => {
	it('returns true on successful safeStorage write', async () => {
		stubBridge({ setToken: async () => ({ ok: true }) });
		expect(await persistAccount('anilist', payload())).toBe(true);
	});

	it('returns false when the bridge rejects the write', async () => {
		stubBridge({ setToken: async () => ({ ok: false, kind: 'io_error' }) });
		expect(await persistAccount('anilist', payload())).toBe(false);
	});

	it('returns false when no bridge is wired', async () => {
		(globalThis as { window?: unknown }).window = undefined;
		expect(await persistAccount('anilist', payload())).toBe(false);
	});
});

describe('clearPersistedAccount', () => {
	it('returns true on successful clear', async () => {
		stubBridge({ clearToken: async () => ({ ok: true }) });
		expect(await clearPersistedAccount('anilist')).toBe(true);
	});

	it('returns false when no bridge is wired', async () => {
		(globalThis as { window?: unknown }).window = undefined;
		expect(await clearPersistedAccount('anilist')).toBe(false);
	});
});

describe('openOAuth', () => {
	it('forwards the auth URL to the bridge', async () => {
		const spy = vi.fn().mockResolvedValue({ ok: true, code: 'c', state: 's' });
		stubBridge({ openOAuth: spy });
		const r = await openOAuth('https://anilist.co/x');
		expect(spy).toHaveBeenCalledWith({ authUrl: 'https://anilist.co/x' });
		expect(r.ok).toBe(true);
	});

	it('returns no_bridge when the preload is missing', async () => {
		(globalThis as { window?: unknown }).window = undefined;
		const r = await openOAuth('https://anilist.co/x');
		expect(r.ok).toBe(false);
		if (!r.ok) expect(r.kind).toBe('no_bridge');
	});
});

describe('cancelOAuth', () => {
	it('forwards to the bridge', async () => {
		const spy = vi.fn().mockResolvedValue(true);
		stubBridge({ cancelOAuth: spy });
		expect(await cancelOAuth()).toBe(true);
		expect(spy).toHaveBeenCalled();
	});

	it('returns false when no bridge is wired', async () => {
		(globalThis as { window?: unknown }).window = undefined;
		expect(await cancelOAuth()).toBe(false);
	});
});

describe('openExternal', () => {
	it('forwards to window.aniGui.openExternal', async () => {
		const spy = vi.fn().mockResolvedValue(true);
		(globalThis as { window?: { aniGui?: unknown } }).window = {
			aniGui: { openExternal: spy }
		};
		expect(await openExternal('https://x')).toBe(true);
		expect(spy).toHaveBeenCalledWith('https://x');
	});

	it('returns false when the bridge is missing', async () => {
		(globalThis as { window?: unknown }).window = undefined;
		expect(await openExternal('https://x')).toBe(false);
	});
});
