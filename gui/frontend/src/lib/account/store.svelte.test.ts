/**
 * Tests for the per-provider account store. The store mirrors the
 * download store shape (`lib/download/store.svelte.ts`): one rune
 * `$state` field per provider, mutated by lifecycle methods, derived
 * `connected` / `hasAny` / `hasErrored` getters.
 *
 * Hydrate is exercised against a stubbed `window.aniGui.account`
 * bridge so we can pin the disconnected/connected/expired branches
 * without touching real Electron safeStorage.
 */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { accountStore } from './store.svelte';
import type { PersistedAccount } from './types';

function payload(overrides: Partial<PersistedAccount> = {}): PersistedAccount {
	return {
		access_token: 'a',
		refresh_token: null,
		expires_at_epoch_s: 1_999_999_999,
		user_id: '7',
		username: 'pucci',
		avatar_url: null,
		...overrides
	};
}

beforeEach(() => {
	// Reset the singleton between tests — its state is module-scoped.
	accountStore.setDisconnected('anilist');
	accountStore.setDisconnected('mal');
	accountStore.setDisconnected('inhouse');
});

afterEach(() => {
	(globalThis as { window?: unknown }).window = undefined;
});

describe('accountStore lifecycle', () => {
	it('starts every provider in disconnected state', () => {
		expect(accountStore.byProvider.anilist.kind).toBe('disconnected');
		expect(accountStore.byProvider.mal.kind).toBe('disconnected');
		expect(accountStore.byProvider.inhouse.kind).toBe('disconnected');
		expect(accountStore.hasAny).toBe(false);
		expect(accountStore.hasErrored).toBe(false);
		expect(accountStore.connected).toEqual([]);
	});

	it('setConnecting flips state to connecting', () => {
		accountStore.setConnecting('anilist');
		expect(accountStore.byProvider.anilist.kind).toBe('connecting');
		expect(accountStore.hasAny).toBe(false);
	});

	it('setConnected populates the account + lastSyncedAt', () => {
		const p = payload();
		accountStore.setConnected('anilist', p);
		const s = accountStore.byProvider.anilist;
		expect(s.kind).toBe('connected');
		if (s.kind === 'connected') {
			expect(s.account).toEqual(p);
			expect(s.lastSyncedAt).not.toBeNull();
		}
		expect(accountStore.hasAny).toBe(true);
		expect(accountStore.connected).toEqual(['anilist']);
	});

	it('setExpired surfaces the prior account', () => {
		const p = payload();
		accountStore.setExpired('anilist', p);
		const s = accountStore.byProvider.anilist;
		expect(s.kind).toBe('expired');
		expect(accountStore.hasErrored).toBe(true);
	});

	it('setError preserves the prior account when available', () => {
		const p = payload();
		accountStore.setConnected('anilist', p);
		accountStore.setError('anilist', 'sync failed');
		const s = accountStore.byProvider.anilist;
		expect(s.kind).toBe('error');
		if (s.kind === 'error') {
			expect(s.account).toEqual(p);
			expect(s.message).toBe('sync failed');
		}
		expect(accountStore.hasErrored).toBe(true);
	});

	it('setError surfaces null account when starting from disconnected', () => {
		accountStore.setError('anilist', 'oh no');
		const s = accountStore.byProvider.anilist;
		if (s.kind === 'error') {
			expect(s.account).toBeNull();
		}
	});

	it('setError preserves the account when prior state was already error-with-account', () => {
		// Codex P2 #3370096597: a second Disconnect attempt that fails
		// token_clear must NOT drop the persisted account from the UI.
		// Without this preservation the page collapses to bare Connect
		// even though the token is still on disk and the user needs
		// Disconnect to retry the clear.
		const p = payload();
		accountStore.setConnected('anilist', p);
		accountStore.setError('anilist', 'first failure');
		accountStore.setError('anilist', 'second failure');
		const s = accountStore.byProvider.anilist;
		expect(s.kind).toBe('error');
		if (s.kind === 'error') {
			expect(s.account).toEqual(p);
			expect(s.message).toBe('second failure');
		}
	});

	it('setDisconnected drops the prior account', () => {
		accountStore.setConnected('anilist', payload());
		accountStore.setDisconnected('anilist');
		expect(accountStore.byProvider.anilist.kind).toBe('disconnected');
		expect(accountStore.hasAny).toBe(false);
	});

	it('markSynced bumps lastSyncedAt without flipping state', () => {
		accountStore.setConnected('anilist', payload());
		const before = accountStore.byProvider.anilist;
		if (before.kind !== 'connected') throw new Error('precondition');
		const beforeTs = before.lastSyncedAt!;
		vi.useFakeTimers();
		vi.advanceTimersByTime(5_000);
		accountStore.markSynced('anilist');
		vi.useRealTimers();
		const after = accountStore.byProvider.anilist;
		if (after.kind !== 'connected') throw new Error('postcondition');
		expect(after.lastSyncedAt).toBeGreaterThanOrEqual(beforeTs);
	});

	it('markSynced on a non-connected provider is a no-op', () => {
		accountStore.markSynced('anilist');
		expect(accountStore.byProvider.anilist.kind).toBe('disconnected');
	});
});

describe('accountStore.hydrate', () => {
	function stubBridge(byProvider: Record<string, PersistedAccount | null>) {
		(globalThis as { window?: { aniGui?: unknown } }).window = {
			aniGui: {
				account: {
					getToken(provider: string) {
						const acc = byProvider[provider];
						return acc ? { ok: true, payload: acc } : { ok: false, kind: 'not_found' };
					}
				}
			}
		};
	}

	function stubBridgeWithReadError(provider: string, kind: string, message?: string) {
		(globalThis as { window?: { aniGui?: unknown } }).window = {
			aniGui: {
				account: {
					getToken(p: string) {
						return p === provider ? { ok: false, kind, message } : { ok: false, kind: 'not_found' };
					}
				}
			}
		};
	}

	it('seeds disconnected when no payload exists', () => {
		stubBridge({});
		accountStore.hydrate();
		expect(accountStore.byProvider.anilist.kind).toBe('disconnected');
	});

	it('seeds connected when a valid payload exists', () => {
		const p = payload();
		stubBridge({ anilist: p });
		accountStore.hydrate();
		expect(accountStore.byProvider.anilist.kind).toBe('connected');
	});

	it('seeds expired when expiry is in the past', () => {
		const p = payload({ expires_at_epoch_s: 1 });
		stubBridge({ anilist: p });
		accountStore.hydrate();
		expect(accountStore.byProvider.anilist.kind).toBe('expired');
	});

	it('seeds connected when expires_at is zero (unknown)', () => {
		// expires_at_epoch_s === 0 means "we don't know"; treat as
		// valid so a missing-expiry round-trip doesn't accidentally
		// trip the expired branch.
		const p = payload({ expires_at_epoch_s: 0 });
		stubBridge({ anilist: p });
		accountStore.hydrate();
		expect(accountStore.byProvider.anilist.kind).toBe('connected');
	});

	it('is a no-op when the preload bridge is absent', () => {
		(globalThis as { window?: unknown }).window = undefined;
		accountStore.setConnected('anilist', payload());
		accountStore.hydrate();
		// hydrate without a bridge resets to disconnected
		expect(accountStore.byProvider.anilist.kind).toBe('disconnected');
	});

	// Codex P2 #3371530183: when the token file is on disk but the OS
	// keychain is unreachable (libsecret missing on Linux, Keychain
	// access denied, etc.), getToken returns { ok: false, kind:
	// 'encryption_unavailable' }. Collapsing that to `null` here would
	// mark the provider `disconnected` even though the credential file
	// is still on disk — the page would then hide the Disconnect
	// action that calls clearToken, and the orphaned file would have
	// no in-app cleanup path. Surface the read failure as an `error`
	// state so the page can render a cleanup affordance.
	it('seeds error state when keychain read fails with encryption_unavailable', () => {
		stubBridgeWithReadError('anilist', 'encryption_unavailable');
		accountStore.hydrate();
		const s = accountStore.byProvider.anilist;
		expect(s.kind).toBe('error');
		if (s.kind === 'error') {
			// No account payload — we couldn't read one — but the page
			// branch for error-with-no-account now offers Disconnect
			// (= clearToken) so the orphan file can be removed.
			expect(s.account).toBeNull();
			expect(s.message.length).toBeGreaterThan(0);
		}
	});

	it('seeds error state when keychain read fails with decrypt_error', () => {
		// Corrupted token file (e.g., partial write, basic_text reject
		// from #3370070913 on a fresh start) — same orphan-cleanup
		// path as encryption_unavailable.
		stubBridgeWithReadError('anilist', 'decrypt_error');
		accountStore.hydrate();
		expect(accountStore.byProvider.anilist.kind).toBe('error');
	});
});

describe('accountStore.refreshExpired', () => {
	afterEach(() => {
		(globalThis as { window?: unknown }).window = undefined;
		vi.unstubAllGlobals();
	});

	it('refreshes an expired-but-refreshable provider back to connected', async () => {
		const setToken = vi.fn().mockResolvedValue({ ok: true });
		(globalThis as { window?: { aniGui?: unknown } }).window = {
			aniGui: { apiBase: 'http://127.0.0.1:0', account: { setToken } }
		};
		vi.stubGlobal(
			'fetch',
			vi.fn().mockResolvedValue({
				ok: true,
				json: async () => ({
					access_token: 'fresh-access',
					refresh_token: 'fresh-rt',
					expires_at_epoch_s: 4_000_000_000
				})
			})
		);
		accountStore.setExpired('mal', payload({ refresh_token: 'stale-rt' }));

		await accountStore.refreshExpired();

		const s = accountStore.byProvider.mal;
		expect(s.kind).toBe('connected');
		if (s.kind === 'connected') expect(s.account.access_token).toBe('fresh-access');
		expect(setToken).toHaveBeenCalledTimes(1);
	});

	it('leaves an expired provider with no refresh token untouched', async () => {
		const fetchSpy = vi.fn();
		vi.stubGlobal('fetch', fetchSpy);
		accountStore.setExpired('anilist', payload({ refresh_token: null }));

		await accountStore.refreshExpired();

		expect(accountStore.byProvider.anilist.kind).toBe('expired');
		expect(fetchSpy).not.toHaveBeenCalled();
	});
});
