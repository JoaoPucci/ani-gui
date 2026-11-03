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
import { refreshAccount } from './refresh-flow';
import { freshBearerFor, __resetInFlightRefreshes } from './fresh-bearer';
import { persistAccount, refreshTokens } from './api';
import type { PersistedAccount } from './types';
import { KEYRING_WAIT_MS } from './keyring-wait';

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
		// A second Disconnect attempt that fails
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
					async getToken(provider: string) {
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
					async getToken(p: string) {
						return p === provider ? { ok: false, kind, message } : { ok: false, kind: 'not_found' };
					}
				}
			}
		};
	}

	it('seeds disconnected when no payload exists', async () => {
		stubBridge({});
		await accountStore.hydrate();
		expect(accountStore.byProvider.anilist.kind).toBe('disconnected');
	});

	it('seeds connected when a valid payload exists', async () => {
		const p = payload();
		stubBridge({ anilist: p });
		await accountStore.hydrate();
		expect(accountStore.byProvider.anilist.kind).toBe('connected');
	});

	it('seeds expired when expiry is in the past', async () => {
		const p = payload({ expires_at_epoch_s: 1 });
		stubBridge({ anilist: p });
		await accountStore.hydrate();
		expect(accountStore.byProvider.anilist.kind).toBe('expired');
	});

	it('seeds connected when expires_at is zero (unknown)', async () => {
		// expires_at_epoch_s === 0 means "we don't know"; treat as
		// valid so a missing-expiry round-trip doesn't accidentally
		// trip the expired branch.
		const p = payload({ expires_at_epoch_s: 0 });
		stubBridge({ anilist: p });
		await accountStore.hydrate();
		expect(accountStore.byProvider.anilist.kind).toBe('connected');
	});

	it('is a no-op when the preload bridge is absent', async () => {
		(globalThis as { window?: unknown }).window = undefined;
		accountStore.setConnected('anilist', payload());
		await accountStore.hydrate();
		// hydrate without a bridge resets to disconnected
		expect(accountStore.byProvider.anilist.kind).toBe('disconnected');
	});

	// When the token file is on disk but the OS
	// keychain is unreachable (libsecret missing on Linux, Keychain
	// access denied, etc.), getToken returns { ok: false, kind:
	// 'encryption_unavailable' }. Collapsing that to `null` here would
	// mark the provider `disconnected` even though the credential file
	// is still on disk — the page would then hide the Disconnect
	// action that calls clearToken, and the orphaned file would have
	// no in-app cleanup path. Surface the read failure as an `error`
	// state so the page can render a cleanup affordance.
	it('seeds error state when keychain read fails with encryption_unavailable', async () => {
		stubBridgeWithReadError('anilist', 'encryption_unavailable');
		await accountStore.hydrate();
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

	it('seeds error state when keychain read fails with decrypt_error', async () => {
		// Corrupted token file (e.g., a partial write) — same orphan-cleanup
		// path as encryption_unavailable.
		stubBridgeWithReadError('anilist', 'decrypt_error');
		await accountStore.hydrate();
		expect(accountStore.byProvider.anilist.kind).toBe('error');
	});

	// The keyring can take its time: a locked one waits on its unlock
	// prompt, an unresponsive secret service until the bus gives up.
	// The reads are asynchronous, the app renders signed out meanwhile,
	// and each provider takes its account when its read answers.
	describe('a keyring that has not answered yet', () => {
		type Answer = (r: unknown) => void;
		function stubSlowBridge() {
			const answers: Partial<Record<string, Answer>> = {};
			(globalThis as { window?: { aniGui?: unknown } }).window = {
				aniGui: {
					account: {
						getToken: (p: string) => new Promise((resolve) => (answers[p] = resolve))
					}
				}
			};
			return (p: string, r: unknown) => answers[p]?.(r);
		}
		const flush = async () => {
			for (let i = 0; i < 5; i++) await Promise.resolve();
		};

		afterEach(() => {
			vi.useRealTimers();
		});

		it('leaves every provider signed out until its read answers', async () => {
			const answer = stubSlowBridge();
			const done = accountStore.hydrate();
			await flush();
			expect(accountStore.byProvider.anilist.kind).toBe('disconnected');
			answer('anilist', { ok: true, payload: payload() });
			await flush();
			expect(accountStore.byProvider.anilist.kind).toBe('connected');
			answer('mal', { ok: false, kind: 'not_found' });
			answer('inhouse', { ok: false, kind: 'not_found' });
			await done;
		});

		it('takes one provider without waiting on another still out', async () => {
			const answer = stubSlowBridge();
			void accountStore.hydrate();
			answer('mal', { ok: true, payload: payload({ username: 'm' }) });
			await flush();
			expect(accountStore.byProvider.mal.kind).toBe('connected');
			expect(accountStore.byProvider.anilist.kind).toBe('disconnected');
		});

		it('drops a read the user overtook while it waited', async () => {
			// A sign-in made while the keyring was still answering is newer
			// than what the read finds on disk.
			const answer = stubSlowBridge();
			void accountStore.hydrate();
			const fresh = payload({ access_token: 'fresh', username: 'new' });
			accountStore.setConnected('anilist', fresh);
			answer('anilist', { ok: true, payload: payload({ access_token: 'stale' }) });
			await flush();
			const s = accountStore.byProvider.anilist;
			expect(s.kind).toBe('connected');
			if (s.kind === 'connected') expect(s.account.access_token).toBe('fresh');
		});

		it('reads a provider again when what overtook its read was undone', async () => {
			// A connect started and cancelled while the keyring was still
			// answering leaves the provider signed out; the sign-in saved on
			// disk is still there and must not stay hidden until relaunch.
			const answer = stubSlowBridge();
			void accountStore.hydrate();
			accountStore.setConnecting('anilist');
			accountStore.setDisconnected('anilist');
			answer('anilist', { ok: true, payload: payload() });
			await flush();
			// The answer was overtaken; the provider is read again.
			expect(accountStore.byProvider.anilist.kind).toBe('disconnected');
			answer('anilist', { ok: true, payload: payload() });
			await flush();
			expect(accountStore.byProvider.anilist.kind).toBe('connected');
		});

		it('says it is waiting on the keyring once a read runs past the wait, and stops when it answers', async () => {
			vi.useFakeTimers();
			const answer = stubSlowBridge();
			const done = accountStore.hydrate();
			answer('mal', { ok: false, kind: 'not_found' });
			answer('inhouse', { ok: false, kind: 'not_found' });
			await vi.advanceTimersByTimeAsync(KEYRING_WAIT_MS - 1);
			expect(accountStore.keyringWaiting).toBe(false);
			await vi.advanceTimersByTimeAsync(1);
			expect(accountStore.keyringWaiting).toBe(true);
			answer('anilist', { ok: true, payload: payload() });
			await done;
			expect(accountStore.keyringWaiting).toBe(false);
			expect(accountStore.byProvider.anilist.kind).toBe('connected');
		});

		it('never says it is waiting when the keyring answers in time', async () => {
			vi.useFakeTimers();
			const answer = stubSlowBridge();
			const done = accountStore.hydrate();
			for (const p of ['anilist', 'mal', 'inhouse']) answer(p, { ok: false, kind: 'not_found' });
			await done;
			await vi.advanceTimersByTimeAsync(KEYRING_WAIT_MS * 2);
			expect(accountStore.keyringWaiting).toBe(false);
		});
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

	// A Disconnect clicked while the boot refresh is
	// awaiting the network must win — beginAccountChange bumps the
	// generation, so the refresh is superseded and the provider is NOT
	// reconnected (nor its token re-persisted) when the call returns.
	it('does not reconnect a provider disconnected mid-refresh', async () => {
		let release: () => void = () => {};
		const gate = new Promise<void>((r) => (release = r));
		const setToken = vi.fn().mockResolvedValue({ ok: true });
		(globalThis as { window?: { aniGui?: unknown } }).window = {
			aniGui: { apiBase: 'http://127.0.0.1:0', account: { setToken } }
		};
		vi.stubGlobal(
			'fetch',
			vi.fn().mockImplementation(async () => {
				await gate;
				return {
					ok: true,
					json: async () => ({ access_token: 'new', refresh_token: 'rt2', expires_at_epoch_s: 9 })
				};
			})
		);
		accountStore.setExpired('mal', payload({ refresh_token: 'rt' }));

		const pending = accountStore.refreshExpired();
		accountStore.beginAccountChange('mal'); // user clicked Disconnect mid-flight
		release();
		await pending;

		expect(accountStore.byProvider.mal.kind).toBe('expired');
		expect(setToken).not.toHaveBeenCalled();
	});
});

describe('accountStore.hydrate triggers refresh', () => {
	afterEach(() => {
		(globalThis as { window?: unknown }).window = undefined;
		vi.restoreAllMocks();
	});

	it('kicks off refreshExpired after seeding state', async () => {
		// Every hydrate path (cold launch AND the
		// account page) must follow up with a refresh, so hydrate() owns it.
		(globalThis as { window?: { aniGui?: unknown } }).window = {
			aniGui: {
				account: {
					getToken: async () => ({ ok: false, kind: 'not_found' })
				}
			}
		};
		const spy = vi.spyOn(accountStore, 'refreshExpired').mockResolvedValue();
		await accountStore.hydrate();
		expect(spy).toHaveBeenCalledTimes(1);
	});
});

describe('accountStore renewal failures', () => {
	it('records what a failed renewal said, per provider', () => {
		accountStore.setExpired('mal', payload({ refresh_token: 'rt' }));
		accountStore.noteRenewalFailed('mal', 'unreachable');
		expect(accountStore.renewalFailure.mal).toBe('unreachable');
		expect(accountStore.renewalFailure.anilist).toBeNull();
	});

	it('forgets it once the sign-in is renewed or reconnected', () => {
		accountStore.setExpired('mal', payload({ refresh_token: 'rt' }));
		accountStore.noteRenewalFailed('mal', 'rejected');
		accountStore.setConnected('mal', payload({ refresh_token: 'rt' }));
		expect(accountStore.renewalFailure.mal).toBeNull();
	});

	it('forgets it when the provider is disconnected', () => {
		accountStore.noteRenewalFailed('mal', 'rejected');
		accountStore.setDisconnected('mal');
		expect(accountStore.renewalFailure.mal).toBeNull();
	});

	it('keeps it while a new sign-in is under way, so a cancelled one restores the notice', () => {
		// The account page restores an expired sign-in after a cancelled
		// or failed connect; with its failure gone the notice would wait
		// for a renewal that never comes.
		accountStore.setExpired('mal', payload({ refresh_token: 'rt' }));
		accountStore.noteRenewalFailed('mal', 'rejected');
		accountStore.setConnecting('mal');
		accountStore.setExpired('mal', payload({ refresh_token: 'rt' }));
		expect(accountStore.renewalFailure.mal).toBe('rejected');
	});

	it('keeps it when the sign-in is marked expired', () => {
		accountStore.noteRenewalFailed('mal', 'unreachable');
		accountStore.setExpired('mal', payload({ refresh_token: 'rt' }));
		expect(accountStore.renewalFailure.mal).toBe('unreachable');
	});
});

describe('accountStore.retryRenewal', () => {
	it('forgets the failed renewals it retries, so their outcome is told again', async () => {
		const refresh = vi.spyOn(accountStore, 'refreshExpired').mockResolvedValue();
		accountStore.setExpired('mal', payload({ refresh_token: 'rt' }));
		accountStore.noteRenewalFailed('mal', 'unreachable');
		await accountStore.retryRenewal();
		expect(accountStore.renewalFailure.mal).toBeNull();
		expect(refresh).toHaveBeenCalledTimes(1);
		refresh.mockRestore();
	});

	it('keeps a refused renewal of a still-valid sign-in: there is nothing to retry', async () => {
		const refresh = vi.spyOn(accountStore, 'refreshExpired').mockResolvedValue();
		accountStore.setConnected('mal', payload({ refresh_token: 'rt' }));
		accountStore.noteRenewalFailed('mal', 'rejected');
		await accountStore.retryRenewal();
		expect(accountStore.renewalFailure.mal).toBe('rejected');
		refresh.mockRestore();
	});
});

describe('accountStore.hydrate after a refused bearer', () => {
	it('keeps a sign-in expired when the keychain hands back the bearer the tracker refused', async () => {
		// The token's date says valid, but the tracker refused it: reading
		// it from the keychain again must not make it look healthy.
		const refused = payload({ refresh_token: 'rt', expires_at_epoch_s: 1_999_999_999 });
		(globalThis as { window?: { aniGui?: unknown } }).window = {
			aniGui: {
				account: {
					getToken: async (p: string) =>
						p === 'mal' ? { ok: true, payload: refused } : { ok: false, kind: 'not_found' }
				}
			}
		};
		const refresh = vi.spyOn(accountStore, 'refreshExpired').mockResolvedValue();
		accountStore.setExpired('mal', refused);
		accountStore.noteRenewalFailed('mal', 'unreachable');
		await accountStore.hydrate();
		expect(accountStore.byProvider.mal.kind).toBe('expired');
		expect(accountStore.renewalFailure.mal).toBe('unreachable');
		refresh.mockRestore();
	});

	it('takes a different bearer from the keychain as the sign-in it is', async () => {
		(globalThis as { window?: { aniGui?: unknown } }).window = {
			aniGui: {
				account: {
					getToken: async (p: string) =>
						p === 'mal'
							? { ok: true, payload: payload({ access_token: 'fresh', refresh_token: 'rt' }) }
							: { ok: false, kind: 'not_found' }
				}
			}
		};
		accountStore.setExpired('mal', payload({ access_token: 'refused', refresh_token: 'rt' }));
		accountStore.noteRenewalFailed('mal', 'rejected');
		await accountStore.hydrate();
		expect(accountStore.byProvider.mal.kind).toBe('connected');
		// A different sign-in owes nothing to the old one's failure.
		expect(accountStore.renewalFailure.mal).toBeNull();
	});
});

describe('accountStore.refreshExpired and a refused renewal', () => {
	it('records what the tracker answered', async () => {
		(globalThis as { window?: { aniGui?: unknown } }).window = {
			aniGui: { apiBase: 'http://127.0.0.1:1' }
		};
		const fetch = vi.fn(
			async () =>
				new Response(
					JSON.stringify({ kind: 'refresh_rejected', key: 'error.account.refresh_rejected' }),
					{ status: 401 }
				)
		);
		vi.stubGlobal('fetch', fetch);
		try {
			accountStore.setExpired('mal', payload({ refresh_token: 'rt', expires_at_epoch_s: 1 }));
			await accountStore.refreshExpired();
			expect(fetch).toHaveBeenCalledTimes(1);
			expect(accountStore.renewalFailure.mal).toBe('rejected');
		} finally {
			vi.unstubAllGlobals();
		}
	});
});

describe('a renewal that a hydrate overtakes', () => {
	// The account page re-reads the keychain while a renewal is out. If
	// the re-read finds a different sign-in, the renewal's outcome is
	// the old one's and must be credited to nothing.
	function race(answer: () => Response) {
		let keychain: PersistedAccount = payload({
			access_token: 'old',
			refresh_token: 'rt',
			expires_at_epoch_s: 1
		});
		const setToken = vi.fn<(p: string, a: PersistedAccount) => Promise<{ ok: boolean }>>(
			async () => ({ ok: true })
		);
		(globalThis as { window?: { aniGui?: unknown } }).window = {
			aniGui: {
				apiBase: 'http://127.0.0.1:1',
				account: {
					getToken: async (p: string) =>
						p === 'mal' ? { ok: true, payload: keychain } : { ok: false, kind: 'not_found' },
					setToken
				}
			}
		};
		let release: () => void = () => {};
		const held = new Promise<void>((resolve) => {
			release = resolve;
		});
		const fetch = vi.fn(async () => {
			await held;
			return answer();
		});
		vi.stubGlobal('fetch', fetch);
		return {
			setKeychain: (a: PersistedAccount) => {
				keychain = a;
			},
			release,
			fetch,
			setToken
		};
	}

	afterEach(() => {
		vi.unstubAllGlobals();
		vi.restoreAllMocks();
	});

	const fresh = payload({ access_token: 'new', refresh_token: 'rt-new' });

	it('does not pin the old renewal’s refusal on the sign-in the re-read found', async () => {
		const r = race(
			() =>
				new Response(
					JSON.stringify({ kind: 'refresh_rejected', key: 'error.account.refresh_rejected' }),
					{ status: 401 }
				)
		);
		accountStore.setExpired(
			'mal',
			payload({ access_token: 'old', refresh_token: 'rt', expires_at_epoch_s: 1 })
		);
		const renewal = accountStore.refreshExpired();
		await Promise.resolve();
		r.setKeychain(fresh);
		vi.spyOn(accountStore, 'refreshExpired').mockResolvedValueOnce();
		await accountStore.hydrate();
		r.release();
		await renewal;
		expect(accountStore.byProvider.mal).toMatchObject({
			kind: 'connected',
			account: { access_token: 'new' }
		});
		expect(accountStore.renewalFailure.mal).toBeNull();
	});

	it('does not overwrite the sign-in the re-read found with the old one renewed', async () => {
		const r = race(() =>
			Response.json({
				access_token: 'old-renewed',
				refresh_token: 'rt2',
				expires_at_epoch_s: 1_999_999_999
			})
		);
		accountStore.setExpired(
			'mal',
			payload({ access_token: 'old', refresh_token: 'rt', expires_at_epoch_s: 1 })
		);
		const renewal = accountStore.refreshExpired();
		await Promise.resolve();
		r.setKeychain(fresh);
		vi.spyOn(accountStore, 'refreshExpired').mockResolvedValueOnce();
		await accountStore.hydrate();
		r.release();
		await renewal;
		expect(accountStore.byProvider.mal).toMatchObject({
			kind: 'connected',
			account: { access_token: 'new' }
		});
		expect(r.setToken).not.toHaveBeenCalled();
	});

	it('still credits a renewal the re-read did not overtake', async () => {
		const r = race(() =>
			Response.json({
				access_token: 'old-renewed',
				refresh_token: 'rt2',
				expires_at_epoch_s: 1_999_999_999
			})
		);
		accountStore.setExpired(
			'mal',
			payload({ access_token: 'old', refresh_token: 'rt', expires_at_epoch_s: 1 })
		);
		const renewal = accountStore.refreshExpired();
		await Promise.resolve();
		// The keychain still holds the same sign-in: the re-read changes nothing.
		vi.spyOn(accountStore, 'refreshExpired').mockResolvedValueOnce();
		await accountStore.hydrate();
		r.release();
		await renewal;
		expect(accountStore.byProvider.mal).toMatchObject({
			kind: 'connected',
			account: { access_token: 'old-renewed' }
		});
	});

	it('still credits a renewal when the re-read finds only that the same sign-in’s date passed', async () => {
		const r = race(() =>
			Response.json({
				access_token: 'renewed',
				refresh_token: 'rt2',
				expires_at_epoch_s: 1_999_999_999
			})
		);
		const soon = payload({
			access_token: 'old',
			refresh_token: 'rt',
			expires_at_epoch_s: 1_999_999_999
		});
		accountStore.setConnected('mal', soon);
		const renewal = refreshAccount(
			{
				refreshTokens,
				persistAccount,
				generation: (p) => accountStore.accountGeneration[p]
			},
			'mal',
			soon
		);
		await Promise.resolve();
		r.setKeychain({ ...soon, expires_at_epoch_s: 1 });
		vi.spyOn(accountStore, 'refreshExpired').mockResolvedValueOnce();
		await accountStore.hydrate();
		expect(accountStore.byProvider.mal.kind).toBe('expired');
		r.release();
		expect((await renewal).kind).toBe('refreshed');
	});

	it('does not write the old renewal over the sign-in a re-read found while its save waited', async () => {
		// The exchange has answered, and the save is queued behind an
		// earlier write for the provider. The re-read lands while it
		// waits: the save must not reach the keychain.
		const r = race(() =>
			Response.json({
				access_token: 'old-renewed',
				refresh_token: 'rt2',
				expires_at_epoch_s: 1_999_999_999
			})
		);
		let releaseWrite: () => void = () => {};
		const writeHeld = new Promise<void>((resolve) => {
			releaseWrite = resolve;
		});
		r.setToken.mockImplementationOnce(async () => {
			await writeHeld;
			return { ok: true };
		});
		const earlier = persistAccount('mal', payload({ access_token: 'earlier' }));
		accountStore.setExpired(
			'mal',
			payload({ access_token: 'old', refresh_token: 'rt', expires_at_epoch_s: 1 })
		);
		const renewal = accountStore.refreshExpired();
		let settled = false;
		void renewal.then(() => {
			settled = true;
		});
		r.release();
		await new Promise((resolve) => setTimeout(resolve, 0));
		r.setKeychain(fresh);
		vi.spyOn(accountStore, 'refreshExpired').mockResolvedValueOnce();
		await accountStore.hydrate();
		await new Promise((resolve) => setTimeout(resolve, 0));
		// Still waiting means its save is in the queue behind the earlier
		// write: the re-read landed where this case says, not before the
		// exchange answered (where the renewal would already be over).
		expect(settled, 'the renewal is waiting to save').toBe(false);
		releaseWrite();
		await earlier;
		await renewal;
		const written = r.setToken.mock.calls.map((c) => c[1].access_token);
		expect(written).toEqual(['earlier']);
		expect(accountStore.byProvider.mal).toMatchObject({
			kind: 'connected',
			account: { access_token: 'new' }
		});
	});

	it('keeps the renewal’s own new bearer when a re-read finds it before the save reports', async () => {
		// The main process has written the renewed token, but the save
		// has not answered yet. A re-read now finds the renewal's own
		// write: the caller must get the renewed bearer, not the old one.
		__resetInFlightRefreshes();
		const r = race(() =>
			Response.json({
				access_token: 'renewed',
				refresh_token: 'rt2',
				expires_at_epoch_s: 1_999_999_999
			})
		);
		let releaseWrite: () => void = () => {};
		const writeHeld = new Promise<void>((resolve) => {
			releaseWrite = resolve;
		});
		r.setToken.mockImplementationOnce(async (_p, written) => {
			r.setKeychain(written);
			await writeHeld;
			return { ok: true };
		});
		const soon = payload({
			access_token: 'old',
			refresh_token: 'rt',
			expires_at_epoch_s: Math.floor(Date.now() / 1000) + 30
		});
		accountStore.setConnected('mal', soon);
		const bearer = freshBearerFor('mal');
		r.release();
		await vi.waitFor(() => expect(r.setToken).toHaveBeenCalled());
		vi.spyOn(accountStore, 'refreshExpired').mockResolvedValueOnce();
		await accountStore.hydrate();
		releaseWrite();
		expect(await bearer).toBe('renewed');
		expect(accountStore.byProvider.mal).toMatchObject({
			kind: 'connected',
			account: { access_token: 'renewed' }
		});
	});
});
