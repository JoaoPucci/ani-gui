import { describe, expect, it, vi } from 'vitest';
import {
	expiredRefreshable,
	freshBearer,
	type FreshBearerDeps,
	needsProactiveRefresh,
	REFRESH_SKEW_SECONDS,
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
			persistAccount: vi.fn().mockResolvedValue({ ok: true }),
			generation: () => 0
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
			persistAccount: vi.fn().mockResolvedValue({ ok: true }),
			generation: () => 0
		};
		const out = await refreshAccount(deps, 'mal', account({ refresh_token: 'keep-me' }));
		expect(out.kind).toBe('refreshed');
		if (out.kind === 'refreshed') expect(out.account.refresh_token).toBe('keep-me');
	});

	it('is unrefreshable with no refresh token (no calls)', async () => {
		const deps: RefreshFlowDeps = {
			refreshTokens: vi.fn(),
			persistAccount: vi.fn(),
			generation: () => 0
		};
		const out = await refreshAccount(deps, 'mal', account({ refresh_token: null }));
		expect(out.kind).toBe('unrefreshable');
		expect(deps.refreshTokens).not.toHaveBeenCalled();
	});

	it('fails (keeps expired) when the refresh call throws', async () => {
		const deps: RefreshFlowDeps = {
			refreshTokens: vi.fn().mockRejectedValue(new Error('401')),
			persistAccount: vi.fn().mockResolvedValue({ ok: true }),
			generation: () => 0
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
			persistAccount: vi.fn().mockResolvedValue({ ok: false, kind: 'keychain_unavailable' }),
			generation: () => 0
		};
		const out = await refreshAccount(deps, 'mal', account());
		expect(out.kind).toBe('failed');
	});

	it('fails (does not throw) when persistAccount rejects', async () => {
		// Codex P2 #3421439995: persistAccount now routes through the
		// token-write queue, which preserves a rejected write for the
		// caller. A rejection must surface as `failed`, not escape
		// refreshAccount and abort the caller's best-effort flow.
		const deps: RefreshFlowDeps = {
			refreshTokens: vi
				.fn()
				.mockResolvedValue({ access_token: 'n', refresh_token: 'r', expires_at_epoch_s: 1 }),
			persistAccount: vi.fn().mockRejectedValue(new Error('io_error')),
			generation: () => 0
		};
		const out = await refreshAccount(deps, 'mal', account());
		expect(out.kind).toBe('failed');
	});

	it('is superseded (no persist) when the generation moves during the refresh', async () => {
		// Codex P2 #3416616176 / #3416668470: the user disconnected or
		// re-authed while refreshTokens was in flight — the provider's
		// generation counter advanced. Must NOT persist the stale account
		// back. The bump simulates a disconnect/re-auth landing mid-await,
		// even before the store's account snapshot catches up.
		let gen = 7;
		const persistAccount = vi.fn().mockResolvedValue({ ok: true });
		const deps: RefreshFlowDeps = {
			refreshTokens: vi.fn().mockImplementation(async () => {
				gen = 8; // a disconnect/re-auth raced the network call
				return { access_token: 'new', refresh_token: 'new-rt', expires_at_epoch_s: 9 };
			}),
			persistAccount,
			generation: () => gen
		};
		const out = await refreshAccount(deps, 'mal', account());
		expect(out.kind).toBe('superseded');
		expect(persistAccount).not.toHaveBeenCalled();
	});

	it('is superseded when the generation moves during the persist await', async () => {
		// Codex P2 #3416732381: a disconnect/re-auth can land in the gap
		// between the post-network check and the persist completing. The
		// post-persist re-check must also catch it, so onRefreshed never
		// reconnects the superseded account.
		let gen = 0;
		const persistAccount = vi.fn().mockImplementation(async () => {
			gen = 1; // disconnect/re-auth raced the safeStorage write
			return { ok: true };
		});
		const deps: RefreshFlowDeps = {
			refreshTokens: vi
				.fn()
				.mockResolvedValue({ access_token: 'new', refresh_token: 'new-rt', expires_at_epoch_s: 9 }),
			persistAccount,
			generation: () => gen
		};
		const out = await refreshAccount(deps, 'mal', account());
		expect(out.kind).toBe('superseded');
	});

	it('persists when the generation is unchanged across the refresh', async () => {
		const persistAccount = vi.fn().mockResolvedValue({ ok: true });
		const deps: RefreshFlowDeps = {
			refreshTokens: vi
				.fn()
				.mockResolvedValue({ access_token: 'new', refresh_token: 'new-rt', expires_at_epoch_s: 9 }),
			persistAccount,
			generation: () => 3
		};
		const out = await refreshAccount(deps, 'mal', account());
		expect(out.kind).toBe('refreshed');
		expect(persistAccount).toHaveBeenCalledTimes(1);
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
			persistAccount: vi.fn().mockResolvedValue({ ok: true }),
			generation: () => 0,
			changing: () => false
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
			persistAccount: vi.fn().mockResolvedValue({ ok: true }),
			generation: () => 0,
			changing: () => false
		});
		expect(onRefreshed).not.toHaveBeenCalled();
	});

	it('skips a provider whose account change is in progress (disconnecting)', async () => {
		// Codex P2 #3421609159: the accountChanging gate must cover the
		// expired-refresh path too, not just freshBearerFor. If the user
		// disconnects an expired MAL account just as hydrate fires its
		// refresh, refreshAccount would capture the already-bumped
		// generation and its queued persist could land after the clear,
		// resurrecting the removed token. A changing provider must be left
		// untouched.
		const byProvider = {
			anilist: { kind: 'disconnected' },
			mal: { kind: 'expired', account: account({ refresh_token: 'rt' }) },
			inhouse: { kind: 'disconnected' }
		} as unknown as Record<Provider, ProviderState>;
		const onRefreshed = vi.fn();
		const refreshTokens = vi
			.fn()
			.mockResolvedValue({ access_token: 'new', refresh_token: 'rt2', expires_at_epoch_s: 9 });
		await refreshExpiredAccounts({
			byProvider: () => byProvider,
			onRefreshed,
			refreshTokens,
			persistAccount: vi.fn().mockResolvedValue({ ok: true }),
			generation: () => 0,
			changing: (p) => p === 'mal'
		});
		expect(refreshTokens).not.toHaveBeenCalled();
		expect(onRefreshed).not.toHaveBeenCalled();
	});
});

describe('needsProactiveRefresh', () => {
	const nowSec = 1_000_000;

	it('is true when a refreshable token expires within the skew window', () => {
		const acct = account({ refresh_token: 'rt', expires_at_epoch_s: nowSec + 60 });
		expect(needsProactiveRefresh(acct, nowSec)).toBe(true);
	});

	it('is false when the token is comfortably in the future', () => {
		const acct = account({
			refresh_token: 'rt',
			expires_at_epoch_s: nowSec + REFRESH_SKEW_SECONDS + 60
		});
		expect(needsProactiveRefresh(acct, nowSec)).toBe(false);
	});

	it('is false without a refresh token (can not be silently refreshed)', () => {
		const acct = account({ refresh_token: null, expires_at_epoch_s: nowSec + 10 });
		expect(needsProactiveRefresh(acct, nowSec)).toBe(false);
	});

	it('is false when the expiry is unknown (<= 0)', () => {
		const acct = account({ refresh_token: 'rt', expires_at_epoch_s: 0 });
		expect(needsProactiveRefresh(acct, nowSec)).toBe(false);
	});

	it('is true once the token is already past expiry', () => {
		const acct = account({ refresh_token: 'rt', expires_at_epoch_s: nowSec - 5 });
		expect(needsProactiveRefresh(acct, nowSec)).toBe(true);
	});
});

describe('freshBearer', () => {
	const nowMs = 1_000_000_000;
	const nowSec = Math.floor(nowMs / 1000);

	function freshDeps(over: Partial<FreshBearerDeps> = {}): FreshBearerDeps {
		return {
			refreshTokens: vi.fn().mockResolvedValue({
				access_token: 'fresh',
				refresh_token: 'rt2',
				expires_at_epoch_s: nowSec + 3600
			}),
			persistAccount: vi.fn().mockResolvedValue({ ok: true }),
			generation: () => 0,
			onRefreshed: vi.fn(),
			now: () => nowMs,
			...over
		};
	}

	it('returns the current bearer untouched when the token is not near expiry', async () => {
		const deps = freshDeps();
		const acct = account({ access_token: 'current', expires_at_epoch_s: nowSec + 3600 });
		const bearer = await freshBearer(deps, 'mal', acct);
		expect(bearer).toBe('current');
		expect(deps.refreshTokens).not.toHaveBeenCalled();
		expect(deps.onRefreshed).not.toHaveBeenCalled();
	});

	it('refreshes a near-expiry token, commits it, and returns the fresh bearer', async () => {
		const deps = freshDeps();
		const acct = account({ access_token: 'old', expires_at_epoch_s: nowSec + 30 });
		const bearer = await freshBearer(deps, 'mal', acct);
		expect(bearer).toBe('fresh');
		expect(deps.refreshTokens).toHaveBeenCalledWith('mal', 'rt');
		expect(deps.onRefreshed).toHaveBeenCalledWith(
			'mal',
			account({ access_token: 'fresh', refresh_token: 'rt2', expires_at_epoch_s: nowSec + 3600 })
		);
	});

	it('falls back to the existing bearer when the refresh fails', async () => {
		const deps = freshDeps({ refreshTokens: vi.fn().mockRejectedValue(new Error('401')) });
		const acct = account({ access_token: 'old', expires_at_epoch_s: nowSec + 30 });
		const bearer = await freshBearer(deps, 'mal', acct);
		expect(bearer).toBe('old');
		expect(deps.onRefreshed).not.toHaveBeenCalled();
	});

	it('falls back to the existing bearer when persistAccount rejects (no throw)', async () => {
		// Codex P2 #3421439995: a rejected safeStorage write must not escape
		// freshBearer and abort the caller's best-effort write-back / refresh.
		const deps = freshDeps({ persistAccount: vi.fn().mockRejectedValue(new Error('io_error')) });
		const acct = account({ access_token: 'old', expires_at_epoch_s: nowSec + 30 });
		const bearer = await freshBearer(deps, 'mal', acct);
		expect(bearer).toBe('old');
		expect(deps.onRefreshed).not.toHaveBeenCalled();
	});

	it('falls back to the existing bearer and does not commit when superseded mid-refresh', async () => {
		let gen = 0;
		const deps = freshDeps({
			refreshTokens: vi.fn().mockImplementation(async () => {
				gen = 1;
				return { access_token: 'fresh', refresh_token: 'rt2', expires_at_epoch_s: nowSec + 3600 };
			}),
			generation: () => gen
		});
		const acct = account({ access_token: 'old', expires_at_epoch_s: nowSec + 30 });
		const bearer = await freshBearer(deps, 'mal', acct);
		expect(bearer).toBe('old');
		expect(deps.onRefreshed).not.toHaveBeenCalled();
	});
});
