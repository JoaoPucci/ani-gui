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
	restoreAfterFailedConnect,
	userIdFor,
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
		persistAccount: vi.fn().mockResolvedValue({ ok: true })
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
		deps.persistAccount = vi.fn().mockResolvedValue({ ok: false, kind: 'io_error' });
		const r = await connectAccount('anilist', deps);
		expect(r.kind).toBe('persist_failed');
	});

	// Codex P2 #3372942245: Linux without a usable keyring (libsecret
	// missing / kwallet locked) makes safeStorage fall back to
	// `basic_text`; main.js refuses to persist and returns
	// `encryption_unavailable`. The flow must thread that kind through
	// so the page can render "install your OS keyring" instead of the
	// generic sign-in error users have no way to act on.
	it('threads the underlying kind into persist_failed.reason', async () => {
		const deps = happyDeps();
		deps.persistAccount = vi.fn().mockResolvedValue({ ok: false, kind: 'encryption_unavailable' });
		const r = await connectAccount('anilist', deps);
		expect(r.kind).toBe('persist_failed');
		if (r.kind === 'persist_failed') expect(r.reason).toBe('encryption_unavailable');
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

describe('userIdFor', () => {
	it('returns the stored user_id for connected state', () => {
		const s: ProviderState = { kind: 'connected', account: payload(), lastSyncedAt: 0 };
		expect(userIdFor(s)).toBe('u7');
	});

	it('returns the stored user_id for expired state', () => {
		const s: ProviderState = { kind: 'expired', account: payload() };
		expect(userIdFor(s)).toBe('u7');
	});

	it('returns the stored user_id for error state with account', () => {
		const s: ProviderState = { kind: 'error', account: payload(), message: 'x' };
		expect(userIdFor(s)).toBe('u7');
	});

	it('returns null for error state with no account', () => {
		const s: ProviderState = { kind: 'error', account: null, message: 'x' };
		expect(userIdFor(s)).toBeNull();
	});

	it('returns null for disconnected and connecting states', () => {
		expect(userIdFor({ kind: 'disconnected' })).toBeNull();
		expect(userIdFor({ kind: 'connecting' })).toBeNull();
	});
});

describe('disconnectAccount', () => {
	function disconnectDeps(): DisconnectFlowDeps {
		return {
			clearPersistedAccount: vi.fn().mockResolvedValue(true),
			dropListCache: vi.fn().mockResolvedValue(undefined),
			dropProviderCache: vi.fn().mockResolvedValue(undefined)
		};
	}

	it('clears safeStorage then drops the cache when ids are known', async () => {
		const deps = disconnectDeps();
		const s: ProviderState = { kind: 'connected', account: payload(), lastSyncedAt: 0 };
		const r = await disconnectAccount('anilist', s, deps);
		expect(r.kind).toBe('ok');
		expect(deps.clearPersistedAccount).toHaveBeenCalledWith('anilist');
		expect(deps.dropListCache).toHaveBeenCalledWith('anilist', 'tok', 'u7');
		expect(deps.dropProviderCache).not.toHaveBeenCalled();
	});

	it('uses dropProviderCache when there is no prior account (Codex P2 #3371658227)', async () => {
		// Orphan-token disconnect: hydrate found the keychain
		// unreadable, so the store has no bearer + no user_id. The
		// per-user dropListCache can't run; fall through to the
		// provider-wide clear so PRIVACY.md's promise still holds.
		const deps = disconnectDeps();
		const r = await disconnectAccount('anilist', { kind: 'disconnected' }, deps);
		expect(r.kind).toBe('ok');
		expect(deps.clearPersistedAccount).toHaveBeenCalled();
		expect(deps.dropListCache).not.toHaveBeenCalled();
		expect(deps.dropProviderCache).toHaveBeenCalledWith('anilist');
	});

	it('uses dropProviderCache for error-with-no-account orphan states', async () => {
		// Same orphan-cleanup path triggered from the unreadable-token
		// error state set by hydrate() in #3371530183.
		const deps = disconnectDeps();
		const s: ProviderState = { kind: 'error', account: null, message: 'Keychain read failed' };
		const r = await disconnectAccount('anilist', s, deps);
		expect(r.kind).toBe('ok');
		expect(deps.dropListCache).not.toHaveBeenCalled();
		expect(deps.dropProviderCache).toHaveBeenCalledWith('anilist');
	});

	it('swallows dropListCache failures — eviction is best-effort', async () => {
		const deps = disconnectDeps();
		deps.dropListCache = vi.fn().mockRejectedValue(new Error('cache 502'));
		const s: ProviderState = { kind: 'connected', account: payload(), lastSyncedAt: 0 };
		const r = await disconnectAccount('anilist', s, deps);
		expect(r.kind).toBe('ok');
	});

	it('swallows dropProviderCache failures on orphan disconnects', async () => {
		// Same best-effort policy as dropListCache — next launch's
		// hydrate retries by leaving the cache rows in place; the
		// renderer doesn't surface this to the user.
		const deps = disconnectDeps();
		deps.dropProviderCache = vi.fn().mockRejectedValue(new Error('cache 502'));
		const r = await disconnectAccount('anilist', { kind: 'disconnected' }, deps);
		expect(r.kind).toBe('ok');
	});

	it('returns token_clear_failed when safeStorage clear fails (Codex P2 #3369988183)', async () => {
		// If clearPersistedAccount returns false the bearer is still on
		// disk and hydrate() will restore the account next launch.
		// Telling the user they're disconnected in that state is a lie.
		const deps = disconnectDeps();
		deps.clearPersistedAccount = vi.fn().mockResolvedValue(false);
		const s: ProviderState = { kind: 'connected', account: payload(), lastSyncedAt: 0 };
		const r = await disconnectAccount('anilist', s, deps);
		expect(r.kind).toBe('token_clear_failed');
	});
});

describe('restoreAfterFailedConnect', () => {
	// Codex P2 #3370011851: failed reconnect should NOT discard the
	// account from the UI when the underlying token is still on disk.
	it('keeps expired-with-account when reconnect fails', () => {
		const s: ProviderState = { kind: 'expired', account: payload() };
		expect(restoreAfterFailedConnect(s)).toEqual(s);
	});

	it('keeps error-with-account when connect-from-error fails again', () => {
		const s: ProviderState = { kind: 'error', account: payload(), message: 'last sync failed' };
		expect(restoreAfterFailedConnect(s)).toEqual(s);
	});

	// Codex P2 #3372887747: an error-without-account state means hydrate
	// detected an orphan token file it couldn't decrypt. The page shows
	// Disconnect so the user can purge the file. If they click Connect
	// from there and OAuth fails (cancel, timeout, api_error), falling
	// through to `disconnected` hides Disconnect while the orphan file
	// is still on disk — the user can no longer act on it without
	// inspecting their filesystem. Keep the orphan-error state instead.
	it('preserves error-no-account orphan state on failed reconnect', () => {
		const s: ProviderState = { kind: 'error', account: null, message: 'x' };
		expect(restoreAfterFailedConnect(s)).toEqual(s);
	});

	it('falls through to disconnected when prior state was disconnected', () => {
		expect(restoreAfterFailedConnect({ kind: 'disconnected' })).toEqual({ kind: 'disconnected' });
	});

	it('falls through to disconnected when prior state was connecting', () => {
		// connecting is a transient state — the user just clicked
		// Connect; if it failed, there's no account to preserve.
		expect(restoreAfterFailedConnect({ kind: 'connecting' })).toEqual({ kind: 'disconnected' });
	});

	it("treats connected as a normal disconnect fall-through (shouldn't happen in practice)", () => {
		// connectAniList sets connecting before calling connectAccount,
		// so this branch is unreachable from the page — kept as a
		// defensive fallback for the type system.
		const s: ProviderState = { kind: 'connected', account: payload(), lastSyncedAt: 0 };
		expect(restoreAfterFailedConnect(s)).toEqual({ kind: 'disconnected' });
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
