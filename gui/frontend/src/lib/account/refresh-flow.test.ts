import { describe, expect, it, vi } from 'vitest';
import {
	expiredRefreshable,
	refreshAccount,
	refreshExpiredAccounts,
	type RefreshFlowDeps
} from './refresh-flow';
import type { PersistedAccount, Provider, ProviderState } from './types';

function account(over: Partial<PersistedAccount> = {}): PersistedAccount {
	return {
		access_token: 'old-access',
		refresh_token: 'rt',
		expires_at_epoch_s: 1,
		user_id: 'u1',
		username: 'name',
		avatar_url: null,
		...over
	};
}

describe('expiredRefreshable', () => {
	it('selects only expired providers that carry a refresh token', () => {
		const byProvider = {
			anilist: { kind: 'expired', account: account({ refresh_token: null }) },
			mal: { kind: 'expired', account: account({ refresh_token: 'rt' }) },
			inhouse: { kind: 'connected', account: account(), lastSyncedAt: null }
		} as unknown as Record<Provider, ProviderState>;
		expect(expiredRefreshable(byProvider)).toEqual(['mal']);
	});
});

describe('refreshAccount', () => {
	it('refreshes, merges the new tokens, and re-persists', async () => {
		const deps: RefreshFlowDeps = {
			refreshTokens: vi.fn().mockResolvedValue({
				access_token: 'new-access',
				refresh_token: 'new-rt',
				expires_at_epoch_s: 999
			}),
			persistAccount: vi.fn().mockResolvedValue({ ok: true })
		};
		const out = await refreshAccount(deps, 'mal', account());
		expect(out.kind).toBe('refreshed');
		expect(deps.refreshTokens).toHaveBeenCalledWith('mal', 'rt');
		const expected = account({
			access_token: 'new-access',
			refresh_token: 'new-rt',
			expires_at_epoch_s: 999
		});
		expect(deps.persistAccount).toHaveBeenCalledWith('mal', expected);
		if (out.kind === 'refreshed') expect(out.account).toEqual(expected);
	});

	it('keeps the existing refresh token when the provider returns none', async () => {
		const deps: RefreshFlowDeps = {
			refreshTokens: vi
				.fn()
				.mockResolvedValue({ access_token: 'new', refresh_token: null, expires_at_epoch_s: 5 }),
			persistAccount: vi.fn().mockResolvedValue({ ok: true })
		};
		const out = await refreshAccount(deps, 'mal', account({ refresh_token: 'keep-me' }));
		expect(out.kind).toBe('refreshed');
		if (out.kind === 'refreshed') expect(out.account.refresh_token).toBe('keep-me');
	});

	it('is unrefreshable with no refresh token (no calls)', async () => {
		const deps: RefreshFlowDeps = {
			refreshTokens: vi.fn(),
			persistAccount: vi.fn()
		};
		const out = await refreshAccount(deps, 'mal', account({ refresh_token: null }));
		expect(out.kind).toBe('unrefreshable');
		expect(deps.refreshTokens).not.toHaveBeenCalled();
	});

	it('fails (keeps expired) when the refresh call throws', async () => {
		const deps: RefreshFlowDeps = {
			refreshTokens: vi.fn().mockRejectedValue(new Error('401')),
			persistAccount: vi.fn().mockResolvedValue({ ok: true })
		};
		const out = await refreshAccount(deps, 'mal', account());
		expect(out.kind).toBe('failed');
		expect(deps.persistAccount).not.toHaveBeenCalled();
	});

	it('fails when re-persisting the refreshed token fails', async () => {
		const deps: RefreshFlowDeps = {
			refreshTokens: vi
				.fn()
				.mockResolvedValue({ access_token: 'n', refresh_token: 'r', expires_at_epoch_s: 1 }),
			persistAccount: vi.fn().mockResolvedValue({ ok: false, kind: 'keychain_unavailable' })
		};
		const out = await refreshAccount(deps, 'mal', account());
		expect(out.kind).toBe('failed');
	});
});

describe('refreshExpiredAccounts', () => {
	it('marks refreshed providers connected, leaves failed ones alone', async () => {
		const byProvider = {
			anilist: { kind: 'connected', account: account(), lastSyncedAt: null },
			mal: { kind: 'expired', account: account({ refresh_token: 'rt' }) },
			inhouse: { kind: 'disconnected' }
		} as unknown as Record<Provider, ProviderState>;
		const onRefreshed = vi.fn();
		await refreshExpiredAccounts({
			byProvider: () => byProvider,
			onRefreshed,
			refreshTokens: vi
				.fn()
				.mockResolvedValue({ access_token: 'new', refresh_token: 'rt2', expires_at_epoch_s: 9 }),
			persistAccount: vi.fn().mockResolvedValue({ ok: true })
		});
		expect(onRefreshed).toHaveBeenCalledTimes(1);
		expect(onRefreshed).toHaveBeenCalledWith(
			'mal',
			account({ access_token: 'new', refresh_token: 'rt2', expires_at_epoch_s: 9 })
		);
	});

	it('does not mark connected when refresh fails', async () => {
		const byProvider = {
			anilist: { kind: 'disconnected' },
			mal: { kind: 'expired', account: account() },
			inhouse: { kind: 'disconnected' }
		} as unknown as Record<Provider, ProviderState>;
		const onRefreshed = vi.fn();
		await refreshExpiredAccounts({
			byProvider: () => byProvider,
			onRefreshed,
			refreshTokens: vi.fn().mockRejectedValue(new Error('boom')),
			persistAccount: vi.fn().mockResolvedValue({ ok: true })
		});
		expect(onRefreshed).not.toHaveBeenCalled();
	});
});
