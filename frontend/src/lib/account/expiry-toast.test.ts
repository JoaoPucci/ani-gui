import { describe, expect, it, vi } from 'vitest';
import fc from 'fast-check';
import {
	detectExpiredProviders,
	detectSignInNotices,
	ExpiryToastTracker,
	signInNotice,
	type ExpirySyncDeps
} from './expiry-toast';
import { NO_RENEWAL_FAILURES, type RenewalFailure } from './renewal-failure';
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
		expect(push).toHaveBeenLastCalledWith({ provider: 'mal', username: 'm', notice: 'expired' });
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

describe('signInNotice', () => {
	const renewable = acc({ refresh_token: 'rt' });
	const final = acc({ refresh_token: null });

	it('says nothing while a renewable sign-in is still being renewed', () => {
		// Before the renewal answers, nothing is known: the sign-in may
		// be renewed in a moment, and a notice now would flash and go.
		expect(signInNotice({ kind: 'expired', account: renewable }, null)).toBeNull();
	});

	it('calls a sign-in with no way to renew it expired', () => {
		expect(signInNotice({ kind: 'expired', account: final }, null)).toBe('expired');
	});

	it.each<[RenewalFailure, string]>([
		['rejected', 'refused'],
		['unreachable', 'unreachable'],
		['unknown', 'unknown']
	])('names a %s renewal of an expired sign-in as %s', (failure, notice) => {
		expect(signInNotice({ kind: 'expired', account: renewable }, failure)).toBe(notice);
	});

	it('tells a still-valid sign-in only that its renewal was refused', () => {
		// A valid token keeps working; only a refused refresh token
		// means the session is about to end.
		const connected: ProviderState = { kind: 'connected', account: renewable, lastSyncedAt: 0 };
		expect(signInNotice(connected, 'rejected')).toBe('refused');
		expect(signInNotice(connected, 'unreachable')).toBeNull();
		expect(signInNotice(connected, 'unknown')).toBeNull();
		expect(signInNotice(connected, null)).toBeNull();
	});

	it('never asks to reconnect unless the tracker refused or the session ran out unrenewably', () => {
		const failure = fc.constantFrom<RenewalFailure | null>(
			'rejected',
			'unreachable',
			'unknown',
			null
		);
		const state = fc.oneof(
			fc.constant<ProviderState>({ kind: 'disconnected' }),
			fc.constant<ProviderState>({ kind: 'connecting' }),
			fc.boolean().map<ProviderState>((rt) => ({
				kind: 'expired',
				account: acc({ refresh_token: rt ? 'rt' : null })
			})),
			fc.boolean().map<ProviderState>((rt) => ({
				kind: 'connected',
				account: acc({ refresh_token: rt ? 'rt' : null }),
				lastSyncedAt: 0
			})),
			fc.constant<ProviderState>({ kind: 'error', account: null, message: 'x' })
		);
		fc.assert(
			fc.property(state, failure, (st, f) => {
				const notice = signInNotice(st, f);
				if (notice === 'refused') return f === 'rejected';
				if (notice === 'expired')
					return st.kind === 'expired' && !st.account.refresh_token && f === null;
				if (notice === 'unreachable') return f === 'unreachable' && st.kind === 'expired';
				if (notice === 'unknown') return f === 'unknown' && st.kind === 'expired';
				return true;
			})
		);
	});
});

describe('ExpiryToastTracker with renewal failures', () => {
	function fakeDeps() {
		let n = 0;
		const push = vi.fn().mockImplementation(() => `toast-${++n}`);
		const dismiss = vi.fn();
		const deps: ExpirySyncDeps = { push, dismiss };
		return { deps, push, dismiss };
	}
	const expiredMal = build({
		mal: { kind: 'expired', account: acc({ refresh_token: 'rt', username: 'm' }) }
	});

	it('waits for the renewal before saying anything', () => {
		const { deps, push } = fakeDeps();
		new ExpiryToastTracker().sync(expiredMal, deps, NO_RENEWAL_FAILURES);
		expect(push).not.toHaveBeenCalled();
	});

	it('pushes the notice the renewal earned, with what it says', () => {
		const { deps, push } = fakeDeps();
		new ExpiryToastTracker().sync(expiredMal, deps, { ...NO_RENEWAL_FAILURES, mal: 'rejected' });
		expect(push).toHaveBeenCalledWith({ provider: 'mal', username: 'm', notice: 'refused' });
	});

	it('shows one toast per incident however often the renewal fails the same way', () => {
		const { deps, push } = fakeDeps();
		const t = new ExpiryToastTracker();
		const failing = { ...NO_RENEWAL_FAILURES, mal: 'unreachable' as const };
		for (let i = 0; i < 5; i += 1) t.sync(expiredMal, deps, { ...failing });
		expect(push).toHaveBeenCalledTimes(1);
	});

	it('replaces the notice when the renewal later says something else', () => {
		const { deps, push, dismiss } = fakeDeps();
		const t = new ExpiryToastTracker();
		t.sync(expiredMal, deps, { ...NO_RENEWAL_FAILURES, mal: 'unreachable' });
		t.sync(expiredMal, deps, { ...NO_RENEWAL_FAILURES, mal: 'rejected' });
		expect(dismiss).toHaveBeenCalledWith('toast-1');
		expect(push).toHaveBeenLastCalledWith({ provider: 'mal', username: 'm', notice: 'refused' });
	});

	it('clears the notice once the sign-in is renewed or reconnected', () => {
		const { deps, dismiss } = fakeDeps();
		const t = new ExpiryToastTracker();
		t.sync(expiredMal, deps, { ...NO_RENEWAL_FAILURES, mal: 'unreachable' });
		t.sync(
			build({ mal: { kind: 'connected', account: acc({ refresh_token: 'rt' }), lastSyncedAt: 0 } }),
			deps,
			NO_RENEWAL_FAILURES
		);
		expect(dismiss).toHaveBeenCalledWith('toast-1');
	});

	it('lists notices in priority order with their kinds', () => {
		expect(
			detectSignInNotices(
				build({
					anilist: { kind: 'expired', account: acc({ username: 'al' }) },
					mal: { kind: 'expired', account: acc({ refresh_token: 'rt', username: 'm' }) }
				}),
				{ ...NO_RENEWAL_FAILURES, mal: 'unknown' }
			)
		).toEqual([
			{ provider: 'anilist', username: 'al', notice: 'expired' },
			{ provider: 'mal', username: 'm', notice: 'unknown' }
		]);
	});
});
