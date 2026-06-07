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
	exchangeCode,
	fetchAndCacheList,
	fetchCachedList,
	fetchMe,
	openExternal,
	openOAuth,
	persistAccount,
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
	it('POSTs the user_id + bearer', async () => {
		const spy = mockFetchJson([]);
		await fetchAndCacheList('anilist', 'u-7', 'tok');
		const [url, init] = spy.mock.calls[0];
		expect(String(url)).toContain('/api/account/list/anilist');
		expect(String((init as RequestInit).body)).toContain('"user_id":"u-7"');
	});
});

describe('fetchCachedList', () => {
	it('builds the query string with the user_id', async () => {
		const spy = mockFetchJson([]);
		await fetchCachedList('anilist', 'u-9');
		const [url] = spy.mock.calls[0];
		expect(String(url)).toContain('/api/account/list/anilist/cached?user_id=u-9');
	});

	it('surfaces fetch failure as AccountApiError', async () => {
		mockFetchJson({}, 404);
		await expect(fetchCachedList('anilist', 'u')).rejects.toBeInstanceOf(AccountApiError);
	});
});

describe('dropListCache', () => {
	it('DELETEs the cache scoped to the user', async () => {
		const spy = mockFetchJson('');
		await dropListCache('anilist', 'u-9');
		const [url, init] = spy.mock.calls[0];
		expect(String(url)).toContain('/api/account/list/anilist/cache?user_id=u-9');
		expect((init as RequestInit).method).toBe('DELETE');
	});

	it('throws AccountApiError on non-2xx', async () => {
		mockFetchJson('boom', 500);
		await expect(dropListCache('anilist', 'u')).rejects.toBeInstanceOf(AccountApiError);
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
	it('returns the payload when the bridge has one', () => {
		stubBridge({ getToken: () => ({ ok: true, payload: payload() }) });
		expect(readPersistedAccount('anilist')?.username).toBe('p');
	});

	it('returns null when the bridge returns not_found', () => {
		stubBridge({ getToken: () => ({ ok: false, kind: 'not_found' }) });
		expect(readPersistedAccount('anilist')).toBeNull();
	});

	it('returns null when no bridge is wired (browser-only dev)', () => {
		(globalThis as { window?: unknown }).window = undefined;
		expect(readPersistedAccount('anilist')).toBeNull();
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
