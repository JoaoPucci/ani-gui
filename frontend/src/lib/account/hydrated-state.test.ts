import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import {
	failuresAfterHydrate,
	failuresAfterRetry,
	hydratedState,
	isExpired,
	isSameBearer,
	isSignInChange,
	withFailure
} from './hydrated-state';
import { NO_RENEWAL_FAILURES, type RenewalFailure, type RenewalFailures } from './renewal-failure';
import type { PersistedAccount, Provider, ProviderState } from './types';

const PROVIDERS: Provider[] = ['anilist', 'mal', 'inhouse'];

function acc(access_token: string, expires_at_epoch_s = 2_000): PersistedAccount {
	return {
		access_token,
		refresh_token: 'rt',
		expires_at_epoch_s,
		user_id: '1',
		username: 'u',
		avatar_url: null
	};
}

const failure = fc.constantFrom<RenewalFailure | null>('rejected', 'unreachable', 'unknown', null);
const failures = fc.record<RenewalFailures>({ anilist: failure, mal: failure, inhouse: failure });
const token = fc.constantFrom('a', 'b');
const state = fc.oneof(
	fc.constant<ProviderState>({ kind: 'disconnected' }),
	fc.constant<ProviderState>({ kind: 'connecting' }),
	token.map<ProviderState>((t) => ({ kind: 'expired', account: acc(t) })),
	token.map<ProviderState>((t) => ({ kind: 'connected', account: acc(t), lastSyncedAt: 0 })),
	fc.constant<ProviderState>({ kind: 'error', account: null, message: 'x' })
);
const states = fc.record<Record<Provider, ProviderState>>({
	anilist: state,
	mal: state,
	inhouse: state
});

describe('isExpired', () => {
	it('treats an unknown expiry as valid and a passed one as expired', () => {
		expect(isExpired(acc('a', 0), 5_000)).toBe(false);
		expect(isExpired(acc('a', 5_000), 5_000)).toBe(true);
		expect(isExpired(acc('a', 5_001), 5_000)).toBe(false);
	});
});

describe('isSameBearer', () => {
	it('compares the bearer a state holds, and finds none in a state without an account', () => {
		expect(isSameBearer({ kind: 'expired', account: acc('a') }, acc('a'))).toBe(true);
		expect(isSameBearer({ kind: 'expired', account: acc('a') }, acc('b'))).toBe(false);
		expect(isSameBearer({ kind: 'error', account: null, message: 'x' }, acc('a'))).toBe(false);
		expect(isSameBearer({ kind: 'disconnected' }, acc('a'))).toBe(false);
	});
});

describe('hydratedState', () => {
	it('seeds connected for a valid token and expired for a passed one', () => {
		const read = (a: PersistedAccount) => ({ ok: true as const, account: a });
		expect(hydratedState({ kind: 'disconnected' }, read(acc('a')), 1_000).kind).toBe('connected');
		expect(hydratedState({ kind: 'disconnected' }, read(acc('a')), 3_000).kind).toBe('expired');
	});

	it('keeps the bearer already marked expired expired, and takes any other as it is', () => {
		fc.assert(
			fc.property(state, token, (prev, t) => {
				const out = hydratedState(prev, { ok: true, account: acc(t) }, 1_000);
				const refused = prev.kind === 'expired' && prev.account.access_token === t;
				return out.kind === (refused ? 'expired' : 'connected');
			})
		);
	});

	it('seeds disconnected for no token, and an error with no account for an unreadable one', () => {
		expect(hydratedState({ kind: 'disconnected' }, { ok: false, kind: 'not_found' }, 1)).toEqual({
			kind: 'disconnected'
		});
		expect(
			hydratedState({ kind: 'disconnected' }, { ok: false, kind: 'unreadable', detail: 'd' }, 1)
		).toEqual({ kind: 'error', account: null, message: 'Keychain read failed: d' });
	});
});

describe('withFailure', () => {
	it('sets one entry and leaves the rest; returns the same object when nothing changes', () => {
		fc.assert(
			fc.property(failures, fc.constantFrom(...PROVIDERS), failure, (fs, p, f) => {
				const out = withFailure(fs, p, f);
				const rest = PROVIDERS.filter((q) => q !== p).every((q) => out[q] === fs[q]);
				return out[p] === f && rest && (fs[p] === f) === (out === fs);
			})
		);
	});
});

describe('failuresAfterHydrate', () => {
	it('forgets a failure only where the keychain handed back a different bearer', () => {
		fc.assert(
			fc.property(states, states, failures, (prev, next, fs) => {
				const out = failuresAfterHydrate(prev, next, fs);
				return PROVIDERS.every((p) => {
					const s = next[p];
					const changed = 'account' in s && !!s.account && !isSameBearer(prev[p], s.account);
					return out[p] === (changed ? null : fs[p]);
				});
			})
		);
	});
});

describe('failuresAfterRetry', () => {
	it('forgets the failure of every expired sign-in and keeps the rest', () => {
		fc.assert(
			fc.property(states, failures, (byProvider, fs) => {
				const out = failuresAfterRetry(byProvider, fs);
				return PROVIDERS.every((p) => out[p] === (byProvider[p].kind === 'expired' ? null : fs[p]));
			})
		);
	});

	it('changes nothing when nothing is expired', () => {
		const fs: RenewalFailures = { ...NO_RENEWAL_FAILURES, mal: 'rejected' };
		expect(
			failuresAfterRetry(
				{
					anilist: { kind: 'disconnected' },
					mal: { kind: 'disconnected' },
					inhouse: { kind: 'disconnected' }
				},
				fs
			)
		).toBe(fs);
	});
});

describe('isSignInChange', () => {
	// Built from a kind and the bearer it carries, so the expected answer
	// comes from the inputs, not from reading the states back.
	const held = fc.record({
		kind: fc.constantFrom('disconnected', 'connecting', 'expired', 'connected', 'error'),
		bearer: token
	});
	function build({ kind, bearer }: { kind: string; bearer: string }): {
		state: ProviderState;
		holds: string | null;
	} {
		if (kind === 'expired') return { state: { kind, account: acc(bearer) }, holds: bearer };
		if (kind === 'connected')
			return { state: { kind, account: acc(bearer), lastSyncedAt: 0 }, holds: bearer };
		if (kind === 'error') return { state: { kind, account: null, message: 'x' }, holds: null };
		return { state: { kind } as ProviderState, holds: null };
	}

	it('is a change exactly when the bearer held moved', () => {
		fc.assert(
			fc.property(held, held, (a, b) => {
				const prev = build(a);
				const next = build(b);
				return isSignInChange(prev.state, next.state) === (prev.holds !== next.holds);
			})
		);
	});

	it('is no change when only the date of the same sign-in passed', () => {
		// A renewal out for this sign-in is still this sign-in's.
		expect(
			isSignInChange(
				{ kind: 'connected', account: acc('a'), lastSyncedAt: 0 },
				{ kind: 'expired', account: acc('a') }
			)
		).toBe(false);
	});
});

describe('isSignInChange across a renewal', () => {
	// A renewal of sign-in A writes bearer B. A re-read can land before
	// the exchange answers, while the save waits in the queue, after the
	// save was sent (the keychain then holds B), or after it settled
	// (memory holds B too). What arrives is either the renewal's own
	// result or a different sign-in, C. Only C is a change.
	type Phase = 'before-exchange' | 'save-queued' | 'save-sent' | 'after-save';
	const phase = fc.constantFrom<Phase>('before-exchange', 'save-queued', 'save-sent', 'after-save');
	const kind = fc.constantFrom<'connected' | 'expired'>('connected', 'expired');
	function at(p: Phase, other: boolean) {
		const memory = p === 'after-save' ? 'B' : 'A';
		const writing = p === 'save-queued' || p === 'save-sent' ? new Set(['B']) : new Set<string>();
		const keychain = other ? 'C' : p === 'save-sent' || p === 'after-save' ? 'B' : 'A';
		return { memory, writing, keychain };
	}
	it('is a change exactly when a different sign-in arrived, at every point of the renewal', () => {
		fc.assert(
			fc.property(phase, fc.boolean(), kind, kind, (p, other, k1, k2) => {
				const { memory, writing, keychain } = at(p, other);
				const prev: ProviderState =
					k1 === 'expired'
						? { kind: 'expired', account: acc(memory) }
						: { kind: 'connected', account: acc(memory), lastSyncedAt: 0 };
				const next: ProviderState =
					k2 === 'expired'
						? { kind: 'expired', account: acc(keychain) }
						: { kind: 'connected', account: acc(keychain), lastSyncedAt: null };
				return isSignInChange(prev, next, writing) === other;
			})
		);
	});

	it('takes the renewal’s own save, read back while it is out, as the same sign-in', () => {
		expect(
			isSignInChange(
				{ kind: 'connected', account: acc('A'), lastSyncedAt: 0 },
				{ kind: 'connected', account: acc('B'), lastSyncedAt: null },
				new Set(['B'])
			)
		).toBe(false);
	});
});
