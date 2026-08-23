import { describe, expect, it, vi } from 'vitest';
import { detectExpiredProviders, ExpiryToastTracker, type ExpirySyncDeps } from './expiry-toast';
import type { PersistedAccount, Provider, ProviderState } from './types';

function acc(over: Partial<PersistedAccount> = {}): PersistedAccount {
	return {
		access_token: 't',
		refresh_token: null,
		expires_at_epoch_s: 0,
		user_id: '1',
		username: 'name',
		avatar_url: null,
		...over
	};
}

function build(over: Partial<Record<Provider, ProviderState>>): Record<Provider, ProviderState> {
	return {
		anilist: { kind: 'disconnected' },
		mal: { kind: 'disconnected' },
		inhouse: { kind: 'disconnected' },
		...over
	};
}

describe('detectExpiredProviders', () => {
	it('returns an empty list when every provider is disconnected', () => {
		expect(detectExpiredProviders(build({}))).toEqual([]);
	});

	it('returns an empty list when every connected session is still valid', () => {
		expect(
			detectExpiredProviders(
				build({
					anilist: { kind: 'connected', account: acc({ username: 'shiro' }), lastSyncedAt: 0 }
				})
			)
		).toEqual([]);
	});

	it('flags an expired AniList session with its username', () => {
		const out = detectExpiredProviders(
			build({ anilist: { kind: 'expired', account: acc({ username: 'shiro' }) } })
		);
		expect(out).toEqual([{ provider: 'anilist', username: 'shiro' }]);
	});

	it('returns expired providers in priority order (AniList → MAL → InHouse)', () => {
		const out = detectExpiredProviders(
			build({
				mal: { kind: 'expired', account: acc({ username: 'mal-user' }) },
				anilist: { kind: 'expired', account: acc({ username: 'al-user' }) }
			})
		);
		expect(out.map((e) => e.provider)).toEqual(['anilist', 'mal']);
	});

	it('does NOT flag an error-with-account state (different surface — chip dot + /account)', () => {
		expect(
			detectExpiredProviders(
				build({
					anilist: { kind: 'error', account: acc({ username: 'shiro' }), message: 'boom' }
				})
			)
		).toEqual([]);
	});

	it('does NOT flag connecting / disconnected providers', () => {
		expect(
			detectExpiredProviders(
				build({ anilist: { kind: 'connecting' }, mal: { kind: 'disconnected' } })
			)
		).toEqual([]);
	});
});

describe('ExpiryToastTracker', () => {
	function fakeDeps(): {
		deps: ExpirySyncDeps;
		push: ReturnType<typeof vi.fn>;
		dismiss: ReturnType<typeof vi.fn>;
	} {
		const push = vi
			.fn()
			.mockImplementation((info: { provider: Provider }) => `toast-${info.provider}`);
		const dismiss = vi.fn();
		return { deps: { push, dismiss }, push, dismiss };
	}

	it('no-op when nothing is expired', () => {
		const { deps, push, dismiss } = fakeDeps();
		new ExpiryToastTracker().sync(build({}), deps);
		expect(push).not.toHaveBeenCalled();
		expect(dismiss).not.toHaveBeenCalled();
	});

	it('pushes one toast per expired provider on first sync', () => {
		const { deps, push } = fakeDeps();
		new ExpiryToastTracker().sync(
			build({
				anilist: { kind: 'expired', account: acc({ username: 'al' }) },
				mal: { kind: 'expired', account: acc({ username: 'm' }) }
			}),
			deps
		);
		expect(push).toHaveBeenCalledTimes(2);
		expect(push.mock.calls[0]?.[0]).toMatchObject({ provider: 'anilist', username: 'al' });
		expect(push.mock.calls[1]?.[0]).toMatchObject({ provider: 'mal', username: 'm' });
	});

	it('does NOT double-push when the same provider stays expired across syncs', () => {
		const { deps, push, dismiss } = fakeDeps();
		const t = new ExpiryToastTracker();
		const state = build({
			anilist: { kind: 'expired', account: acc({ username: 'al' }) }
		});
		t.sync(state, deps);
		t.sync(state, deps);
		expect(push).toHaveBeenCalledTimes(1);
		expect(dismiss).not.toHaveBeenCalled();
	});

	it('dismisses a tracked toast when the provider recovers (expired → connected)', () => {
		const { deps, push, dismiss } = fakeDeps();
		const t = new ExpiryToastTracker();
		t.sync(build({ anilist: { kind: 'expired', account: acc({ username: 'al' }) } }), deps);
		t.sync(
			build({
				anilist: { kind: 'connected', account: acc({ username: 'al' }), lastSyncedAt: 0 }
			}),
			deps
		);
		expect(dismiss).toHaveBeenCalledWith('toast-anilist');
		expect(push).toHaveBeenCalledTimes(1);
	});

	it('dismisses a tracked toast when the provider is disconnected', () => {
		const { deps, dismiss } = fakeDeps();
		const t = new ExpiryToastTracker();
		t.sync(build({ anilist: { kind: 'expired', account: acc({ username: 'al' }) } }), deps);
		t.sync(build({}), deps);
		expect(dismiss).toHaveBeenCalledWith('toast-anilist');
	});

	it('handles mixed transitions (one recovers, another newly expires) in a single sync', () => {
		const { deps, push, dismiss } = fakeDeps();
		const t = new ExpiryToastTracker();
		t.sync(build({ anilist: { kind: 'expired', account: acc({ username: 'al' }) } }), deps);
		t.sync(
			build({
				anilist: { kind: 'connected', account: acc({ username: 'al' }), lastSyncedAt: 0 },
				mal: { kind: 'expired', account: acc({ username: 'm' }) }
			}),
			deps
		);
		expect(dismiss).toHaveBeenCalledWith('toast-anilist');
		expect(push).toHaveBeenLastCalledWith({ provider: 'mal', username: 'm' });
	});

	it('re-pushes after a provider recovers and expires again later', () => {
		const { deps, push } = fakeDeps();
		const t = new ExpiryToastTracker();
		const expired = build({
			anilist: { kind: 'expired', account: acc({ username: 'al' }) }
		});
		const connected = build({
			anilist: { kind: 'connected', account: acc({ username: 'al' }), lastSyncedAt: 0 }
		});
		t.sync(expired, deps);
		t.sync(connected, deps);
		t.sync(expired, deps);
		expect(push).toHaveBeenCalledTimes(2);
	});
});
