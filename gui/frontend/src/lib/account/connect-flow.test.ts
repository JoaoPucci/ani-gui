/**
 * Tests for the extracted connect / disconnect flow. The flow is a
 * pure function over its `deps` argument, so we plug in mocks for
 * every I/O call and assert the discriminated-result shape.
 */

import { describe, expect, it, vi } from 'vitest';
import {
	bearerFor,
	connectAccount,
	connectErrorKey,
	disconnectAccount,
	type ConnectFlowDeps,
	type DisconnectFlowDeps
} from './connect-flow';
import type { PersistedAccount, ProviderState } from './types';

function payload(): PersistedAccount {
	return {
		access_token: 'tok',
		refresh_token: null,
		expires_at_epoch_s: 999,
		user_id: 'u7',
		username: 'pucci',
		avatar_url: null
	};
}

function happyDeps(): ConnectFlowDeps {
	return {
		generateState: () => 'state-abc',
		generatePkce: () => ({ verifier: 'v', challenge: 'c', method: 'S256' }),
		buildAuthUrl: vi.fn().mockResolvedValue({ url: 'https://anilist.co/x' }),
		openOAuth: vi.fn().mockResolvedValue({ ok: true, code: 'code-xyz', state: 'state-abc' }),
		exchangeCode: vi.fn().mockResolvedValue({
			access_token: 'tok',
			refresh_token: null,
			expires_at_epoch_s: 999
		}),
		fetchMe: vi.fn().mockResolvedValue({ user_id: 'u7', username: 'pucci', avatar_url: null }),
		persistAccount: vi.fn().mockResolvedValue(true)
	};
}

describe('connectAccount', () => {
	it('returns connected with the persisted account on happy path', async () => {
		const r = await connectAccount('anilist', happyDeps());
		expect(r.kind).toBe('connected');
		if (r.kind === 'connected') {
			expect(r.account.username).toBe('pucci');
			expect(r.account.user_id).toBe('u7');
		}
	});

	it('surfaces openOAuth.kind as oauth_error result', async () => {
		const deps = happyDeps();
		deps.openOAuth = vi.fn().mockResolvedValue({ ok: false, kind: 'port_busy' });
		const r = await connectAccount('anilist', deps);
		expect(r.kind).toBe('oauth_error');
		if (r.kind === 'oauth_error') expect(r.reason).toBe('port_busy');
	});

	it('detects state mismatch when the callback returns a different state', async () => {
		const deps = happyDeps();
		deps.openOAuth = vi.fn().mockResolvedValue({ ok: true, code: 'code', state: 'evil-state' });
		const r = await connectAccount('anilist', deps);
		expect(r.kind).toBe('state_mismatch');
	});

	it('returns persist_failed when safeStorage rejects the write', async () => {
		const deps = happyDeps();
		deps.persistAccount = vi.fn().mockResolvedValue(false);
		const r = await connectAccount('anilist', deps);
		expect(r.kind).toBe('persist_failed');
	});

	it('surfaces buildAuthUrl errors as api_error', async () => {
		const deps = happyDeps();
		deps.buildAuthUrl = vi.fn().mockRejectedValue(Object.assign(new Error('x'), { status: 502 }));
		const r = await connectAccount('anilist', deps);
		expect(r.kind).toBe('api_error');
		if (r.kind === 'api_error') expect(r.status).toBe(502);
	});

	it('surfaces exchangeCode errors as api_error', async () => {
		const deps = happyDeps();
		deps.exchangeCode = vi.fn().mockRejectedValue(new Error('boom'));
		const r = await connectAccount('anilist', deps);
		expect(r.kind).toBe('api_error');
		if (r.kind === 'api_error') expect(r.status).toBeUndefined();
	});

	it('surfaces fetchMe errors as api_error', async () => {
		const deps = happyDeps();
		deps.fetchMe = vi.fn().mockRejectedValue(Object.assign(new Error('x'), { status: 401 }));
		const r = await connectAccount('anilist', deps);
		expect(r.kind).toBe('api_error');
		if (r.kind === 'api_error') expect(r.status).toBe(401);
	});
});

describe('bearerFor', () => {
	it('returns ids for connected state', () => {
		const s: ProviderState = { kind: 'connected', account: payload(), lastSyncedAt: 0 };
		expect(bearerFor(s)).toBe('tok');
	});

	it('returns ids for expired state', () => {
		const s: ProviderState = { kind: 'expired', account: payload() };
		expect(bearerFor(s)).toBe('tok');
	});

	it('returns ids for error state when account is present', () => {
		const s: ProviderState = { kind: 'error', account: payload(), message: 'x' };
		expect(bearerFor(s)).toBe('tok');
	});

	it('returns null for error state with no account', () => {
		const s: ProviderState = { kind: 'error', account: null, message: 'x' };
		expect(bearerFor(s)).toBeNull();
	});

	it('returns null for disconnected state', () => {
		expect(bearerFor({ kind: 'disconnected' })).toBeNull();
	});

	it('returns null for connecting state', () => {
		expect(bearerFor({ kind: 'connecting' })).toBeNull();
	});
});

describe('disconnectAccount', () => {
	function disconnectDeps(): DisconnectFlowDeps {
		return {
			clearPersistedAccount: vi.fn().mockResolvedValue(true),
			dropListCache: vi.fn().mockResolvedValue(undefined)
		};
	}

	it('clears safeStorage then drops the cache when ids are known', async () => {
		const deps = disconnectDeps();
		const s: ProviderState = { kind: 'connected', account: payload(), lastSyncedAt: 0 };
		await disconnectAccount('anilist', s, deps);
		expect(deps.clearPersistedAccount).toHaveBeenCalledWith('anilist');
		expect(deps.dropListCache).toHaveBeenCalledWith('anilist', 'tok');
	});

	it('skips dropListCache when there is no prior account', async () => {
		const deps = disconnectDeps();
		await disconnectAccount('anilist', { kind: 'disconnected' }, deps);
		expect(deps.clearPersistedAccount).toHaveBeenCalled();
		expect(deps.dropListCache).not.toHaveBeenCalled();
	});

	it('swallows dropListCache failures — eviction is best-effort', async () => {
		const deps = disconnectDeps();
		deps.dropListCache = vi.fn().mockRejectedValue(new Error('cache 502'));
		const s: ProviderState = { kind: 'connected', account: payload(), lastSyncedAt: 0 };
		await expect(disconnectAccount('anilist', s, deps)).resolves.toBeUndefined();
	});
});

describe('connectErrorKey', () => {
	it.each([
		['port_busy', 'port_busy'],
		['timeout', 'timeout'],
		['cancelled', 'cancelled'],
		['oauth_error', 'oauth_error'],
		['no_bridge', 'no_bridge'],
		['wat', 'unknown']
	])('maps %s → %s', (input, expected) => {
		expect(connectErrorKey(input)).toBe(expected);
	});
});
