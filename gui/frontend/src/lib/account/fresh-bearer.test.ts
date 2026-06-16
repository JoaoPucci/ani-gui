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
		setConnected: vi.fn((p: string, account: PersistedAccount) => {
			store.byProvider[p] = { kind: 'connected', account };
		})
	}
}));
vi.mock('./api', () => ({ refreshTokens, persistAccount }));
vi.mock('./store.svelte', () => ({ accountStore: store }));

import { freshBearerFor } from './fresh-bearer';

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
});
