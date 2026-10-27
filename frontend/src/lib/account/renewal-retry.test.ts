import { describe, expect, it, vi } from 'vitest';
import fc from 'fast-check';
import { needsRenewalRetry, RenewalRetry, retryDelayMs } from './renewal-retry';
import { NO_RENEWAL_FAILURES, type RenewalFailure } from './renewal-failure';
import type { PersistedAccount, Provider, ProviderState } from './types';

function acc(refresh_token: string | null): PersistedAccount {
	return {
		access_token: 'a',
		refresh_token,
		expires_at_epoch_s: 1,
		user_id: '1',
		username: 'u',
		avatar_url: null
	};
}

function states(mal: ProviderState): Record<Provider, ProviderState> {
	return { anilist: { kind: 'disconnected' }, mal, inhouse: { kind: 'disconnected' } };
}

describe('needsRenewalRetry', () => {
	it('is true while an expired, renewable sign-in waits on a tracker that did not answer', () => {
		expect(
			needsRenewalRetry(states({ kind: 'expired', account: acc('rt') }), {
				...NO_RENEWAL_FAILURES,
				mal: 'unreachable'
			})
		).toBe(true);
	});

	it.each<[string, ProviderState, RenewalFailure | null]>([
		['the tracker refused the token', { kind: 'expired', account: acc('rt') }, 'rejected'],
		['the failure is not known', { kind: 'expired', account: acc('rt') }, 'unknown'],
		['no renewal has failed', { kind: 'expired', account: acc('rt') }, null],
		['there is no refresh token', { kind: 'expired', account: acc(null) }, 'unreachable'],
		[
			'the sign-in is still valid',
			{ kind: 'connected', account: acc('rt'), lastSyncedAt: 0 },
			'unreachable'
		],
		['the provider was disconnected', { kind: 'disconnected' }, 'unreachable']
	])('is false when %s', (_name, mal, failure) => {
		expect(needsRenewalRetry(states(mal), { ...NO_RENEWAL_FAILURES, mal: failure })).toBe(false);
	});
});

describe('retryDelayMs', () => {
	it('starts at thirty seconds and doubles', () => {
		expect([0, 1, 2, 3].map(retryDelayMs)).toEqual([30_000, 60_000, 120_000, 240_000]);
	});

	it('never waits less than before, and never more than ten minutes', () => {
		fc.assert(
			fc.property(fc.nat({ max: 10_000 }), (attempt) => {
				const here = retryDelayMs(attempt);
				const next = retryDelayMs(attempt + 1);
				return here >= 30_000 && next >= here && next <= 600_000;
			})
		);
	});
});

function harness(needs: () => boolean) {
	const timers: { fn: () => void; ms: number; cleared: boolean }[] = [];
	let settle: () => void = () => {};
	const retry = vi.fn(
		() =>
			new Promise<void>((resolve) => {
				settle = resolve;
			})
	);
	const r = new RenewalRetry({
		needsRetry: needs,
		retry,
		setTimer: (fn, ms) => {
			const t = { fn, ms, cleared: false };
			timers.push(t);
			return t;
		},
		clearTimer: (h) => {
			(h as { cleared: boolean }).cleared = true;
		}
	});
	return { r, timers, retry, settle: () => settle() };
}

describe('RenewalRetry', () => {
	it('schedules nothing while no renewal waits', () => {
		const { r, timers } = harness(() => false);
		r.sync();
		expect(timers).toHaveLength(0);
	});

	it('keeps one retry scheduled however often it is told to look', () => {
		const { r, timers } = harness(() => true);
		r.sync();
		r.sync();
		r.sync();
		expect(timers).toHaveLength(1);
		expect(timers[0].ms).toBe(30_000);
	});

	it('retries when the timer fires, and backs off for the next one', async () => {
		const { r, timers, retry, settle } = harness(() => true);
		r.sync();
		timers[0].fn();
		expect(retry).toHaveBeenCalledTimes(1);
		// While the retry is out, nothing else is scheduled.
		r.sync();
		expect(timers).toHaveLength(1);
		settle();
		await Promise.resolve();
		await Promise.resolve();
		expect(timers).toHaveLength(2);
		expect(timers[1].ms).toBe(60_000);
	});

	it('cancels the pending retry once the renewal no longer waits, and starts over next time', () => {
		let needs = true;
		const { r, timers } = harness(() => needs);
		r.sync();
		needs = false;
		r.sync();
		expect(timers[0].cleared).toBe(true);
		needs = true;
		r.sync();
		expect(timers[1].ms).toBe(30_000);
	});
});
