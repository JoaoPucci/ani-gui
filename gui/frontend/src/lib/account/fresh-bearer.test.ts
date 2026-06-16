import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { PersistedAccount } from './types';

// Mock the store + api collaborators so the unit under test is the
// fresh-bearer wiring (+ the real refresh-flow.freshBearer it delegates
// to). The store mock exposes just the surface fresh-bearer touches.
const { refreshTokens, persistAccount, store } = vi.hoisted(() => ({
	refreshTokens: vi.fn(),
	persistAccount: vi.fn(async () => ({ ok: true }) as { ok: true }),
	store: {
		byProvider: {} as Record<string, { kind: string; account?: PersistedAccount }>,
		accountGeneration: { anilist: 0, mal: 0, inhouse: 0 } as Record<string, number>,
		accountChanging: { anilist: false, mal: false, inhouse: false } as Record<string, boolean>,
		setConnected: vi.fn((p: string, account: PersistedAccount) => {
			store.byProvider[p] = { kind: 'connected', account };
		})
	}
}));
vi.mock('./api', () => ({ refreshTokens, persistAccount }));
vi.mock('./store.svelte', () => ({ accountStore: store }));

import { __resetInFlightRefreshes, freshBearerFor } from './fresh-bearer';

function account(over: Partial<PersistedAccount> = {}): PersistedAccount {
	return {
		access_token: 'old',
		refresh_token: 'rt',
		expires_at_epoch_s: 4_000_000_000,
		user_id: 'u',
		username: 'name',
		avatar_url: null,
		...over
	};
}

const nowSec = () => Math.floor(Date.now() / 1000);

beforeEach(() => {
	refreshTokens.mockReset();
	persistAccount.mockClear();
	store.setConnected.mockClear();
	for (const k of Object.keys(store.byProvider)) delete store.byProvider[k];
	store.accountGeneration = { anilist: 0, mal: 0, inhouse: 0 };
	store.accountChanging = { anilist: false, mal: false, inhouse: false };
	__resetInFlightRefreshes();
});

afterEach(() => vi.useRealTimers());

describe('freshBearerFor', () => {
	it('returns null for a disconnected provider without refreshing', async () => {
		store.byProvider.mal = { kind: 'disconnected' };
		expect(await freshBearerFor('mal')).toBeNull();
		expect(refreshTokens).not.toHaveBeenCalled();
	});

	it('returns the current bearer untouched when the token is far from expiry', async () => {
		store.byProvider.mal = { kind: 'connected', account: account({ access_token: 'current' }) };
		expect(await freshBearerFor('mal')).toBe('current');
		expect(refreshTokens).not.toHaveBeenCalled();
	});

	it('refreshes a near-expiry connected token, commits it, returns the fresh bearer', async () => {
		refreshTokens.mockResolvedValue({
			access_token: 'fresh',
			refresh_token: 'rt2',
			expires_at_epoch_s: nowSec() + 3600
		});
		store.byProvider.mal = {
			kind: 'connected',
			account: account({ access_token: 'old', expires_at_epoch_s: nowSec() + 30 })
		};
		const bearer = await freshBearerFor('mal');
		expect(bearer).toBe('fresh');
		expect(refreshTokens).toHaveBeenCalledTimes(1);
		expect(store.setConnected).toHaveBeenCalledTimes(1);
	});

	it('coalesces concurrent refreshes for the same provider into one exchange', async () => {
		// Codex P2 #3420173434: two call sites (Watch Later refresh + a
		// watched-progress sync) hitting the same near-expiry provider must
		// share ONE refresh-token exchange, not start two independent
		// rotations whose writes race.
		let release!: (v: unknown) => void;
		refreshTokens.mockImplementation(
			() =>
				new Promise((r) => {
					release = r;
				})
		);
		store.byProvider.mal = {
			kind: 'connected',
			account: account({ access_token: 'old', expires_at_epoch_s: nowSec() + 30 })
		};

		const p1 = freshBearerFor('mal');
		const p2 = freshBearerFor('mal');
		await Promise.resolve(); // let both reach the in-flight check
		release({ access_token: 'fresh', refresh_token: 'rt2', expires_at_epoch_s: nowSec() + 3600 });
		const [b1, b2] = await Promise.all([p1, p2]);

		expect(b1).toBe('fresh');
		expect(b2).toBe('fresh');
		expect(refreshTokens).toHaveBeenCalledTimes(1);
		expect(store.setConnected).toHaveBeenCalledTimes(1);
	});

	it('does not coalesce across different providers', async () => {
		refreshTokens.mockResolvedValue({
			access_token: 'fresh',
			refresh_token: 'rt2',
			expires_at_epoch_s: nowSec() + 3600
		});
		for (const p of ['mal', 'anilist'] as const) {
			store.byProvider[p] = {
				kind: 'connected',
				account: account({ access_token: 'old', expires_at_epoch_s: nowSec() + 30 })
			};
		}
		await Promise.all([freshBearerFor('mal'), freshBearerFor('anilist')]);
		expect(refreshTokens).toHaveBeenCalledTimes(2);
	});

	it('does not start a refresh while an account change (disconnect) is in progress', async () => {
		// Codex P2 #3421338541: disconnectAccount calls beginAccountChange()
		// (generation bumped, accountChanging set) but leaves byProvider as
		// `connected` until the async clear finishes. A refresh entering that
		// window sees a stable already-bumped generation, so its checks pass
		// and it could enqueue a setToken that the FIFO writes after the
		// disconnect's clearToken — resurrecting the removed token. Treat an
		// in-progress change as non-refreshable: return the current bearer,
		// start no refresh.
		refreshTokens.mockResolvedValue({
			access_token: 'fresh',
			refresh_token: 'rt2',
			expires_at_epoch_s: nowSec() + 3600
		});
		store.accountChanging.mal = true;
		store.byProvider.mal = {
			kind: 'connected',
			account: account({ access_token: 'old', expires_at_epoch_s: nowSec() + 30 })
		};
		const bearer = await freshBearerFor('mal');
		expect(bearer).toBe('old');
		expect(refreshTokens).not.toHaveBeenCalled();
	});

	it('does not coalesce onto a pending refresh after the generation changes mid-flight', async () => {
		// Codex P2 #3420249568: a refresh is in flight for the current
		// connection; the user reconnects the same provider (generation
		// bumps, new account). A subsequent caller must NOT reuse the old
		// promise — when that resolves `superseded`, freshBearer falls back
		// to the PREVIOUS account's bearer, which would be sent for the new
		// session (wrong user). The later caller starts its own refresh.
		let release!: (v: unknown) => void;
		refreshTokens.mockImplementationOnce(
			() =>
				new Promise((r) => {
					release = r;
				})
		);
		store.accountGeneration.mal = 0;
		store.byProvider.mal = {
			kind: 'connected',
			account: account({ access_token: 'old', expires_at_epoch_s: nowSec() + 30 })
		};
		const p1 = freshBearerFor('mal'); // starts refresh at generation 0
		await Promise.resolve();

		// User reconnects mid-flight: generation advances, new session account.
		store.accountGeneration.mal = 1;
		store.byProvider.mal = {
			kind: 'connected',
			account: account({ access_token: 'new-session', expires_at_epoch_s: nowSec() + 30 })
		};
		refreshTokens.mockResolvedValue({
			access_token: 'new-fresh',
			refresh_token: 'rt3',
			expires_at_epoch_s: nowSec() + 3600
		});

		const bearer2 = await freshBearerFor('mal'); // generation 1 → own refresh
		expect(bearer2).toBe('new-fresh');
		expect(refreshTokens).toHaveBeenCalledTimes(2);

		release({ access_token: 'stale', refresh_token: 'rtX', expires_at_epoch_s: nowSec() + 3600 });
		await p1;
	});

	it('refreshes again on a later call once the in-flight refresh has settled', async () => {
		refreshTokens.mockResolvedValue({
			access_token: 'fresh',
			refresh_token: 'rt2',
			expires_at_epoch_s: nowSec() + 30 // still near expiry → next call refreshes again
		});
		store.byProvider.mal = {
			kind: 'connected',
			account: account({ access_token: 'old', expires_at_epoch_s: nowSec() + 30 })
		};
		await freshBearerFor('mal');
		await freshBearerFor('mal');
		expect(refreshTokens).toHaveBeenCalledTimes(2);
	});
});
