import { describe, expect, it } from 'vitest';
import * as fc from 'fast-check';
import { AccountApiError } from './api';
import { classifyReadFailure, planReadRetry, type ReadFailure } from './read-retry';

import { wire } from '$lib/testing/wire-answers';
import type { AniErrorPayload } from '../api';
/** A failed attempt that was not a rate limit. */
const plain: ReadFailure = { rateLimited: false, retryAfterSecs: null, envelope: null };

describe('planReadRetry', () => {
	it('backs off exponentially from 1s', () => {
		expect(planReadRetry([plain])).toBe(1000);
		expect(planReadRetry([plain, plain])).toBe(2000);
		expect(planReadRetry([plain, plain, plain])).toBe(4000);
		expect(planReadRetry([plain, plain, plain, plain])).toBe(8000);
	});

	it('returns null once retries are exhausted (so the caller stops and shows an error)', () => {
		expect(planReadRetry(Array<ReadFailure>(5).fill(plain))).toBeNull();
		expect(planReadRetry(Array<ReadFailure>(10).fill(plain))).toBeNull();
	});

	it('plans the first step before any failure has been recorded', () => {
		expect(planReadRetry([])).toBe(1000);
	});
});

describe('classifyReadFailure', () => {
	it('reads a 429 with the wait its envelope carries', () => {
		const e = new AccountApiError(
			429,
			wire<AniErrorPayload>({
				kind: 'rate_limited',
				retry_after_secs: 60,
				key: 'error.network.rate_limited'
			})
		);
		expect(classifyReadFailure(e)).toEqual({
			rateLimited: true,
			retryAfterSecs: 60,
			envelope: { kind: 'rate_limited', retry_after_secs: 60, key: 'error.network.rate_limited' }
		});
	});

	it('reads a 429 whose envelope names no wait', () => {
		const e = new AccountApiError(
			429,
			wire<AniErrorPayload>({
				kind: 'rate_limited',
				retry_after_secs: null,
				key: 'error.network.rate_limited'
			})
		);
		const got = classifyReadFailure(e);
		expect(got.rateLimited).toBe(true);
		expect(got.retryAfterSecs).toBeNull();
		expect(got.envelope).not.toBeNull();
	});

	it('reads a 429 with no readable body as a rate limit with no envelope', () => {
		expect(classifyReadFailure(new AccountApiError(429, ''))).toEqual({
			rateLimited: true,
			retryAfterSecs: null,
			envelope: null
		});
		expect(classifyReadFailure(new AccountApiError(429, '"just a string"')).envelope).toBeNull();
	});

	it('reads every other failure as a plain one', () => {
		expect(classifyReadFailure(new AccountApiError(502, '{"kind":"upstream"}'))).toEqual(plain);
		expect(classifyReadFailure(new Error('no bearer for anilist'))).toEqual(plain);
		expect(classifyReadFailure(undefined)).toEqual(plain);
	});
});

/** A failed attempt the tracker answered with 429 and `secs` to wait. */
const limited = (secs: number | null): ReadFailure => ({
	rateLimited: true,
	retryAfterSecs: secs,
	envelope: { kind: 'rate_limited', retry_after_secs: secs, key: 'error.network.rate_limited' }
});

describe('planReadRetry on a rate limit', () => {
	// AniList allows this machine 30 requests a minute and answers a 429
	// with the seconds left in the window. Retrying at 1, 2, 4 and 8 s
	// spends four more of next minute's budget to be told the same thing;
	// the plan waits what the tracker asked, once.
	it('waits the advertised seconds before one more attempt', () => {
		expect(planReadRetry([limited(60)])).toBe(60_000);
		expect(planReadRetry([limited(7)])).toBe(7_000);
	});

	it('waits a default when the tracker named no wait', () => {
		expect(planReadRetry([limited(null)])).toBe(30_000);
	});

	it('caps the wait at a minute and never waits less than a second', () => {
		expect(planReadRetry([limited(600)])).toBe(60_000);
		expect(planReadRetry([limited(0)])).toBe(1_000);
	});

	it('gives up on a second rate limit, wherever it falls', () => {
		expect(planReadRetry([limited(60), limited(60)])).toBeNull();
		expect(planReadRetry([plain, limited(5), limited(5)])).toBeNull();
		expect(planReadRetry([limited(5), plain, limited(5)])).toBeNull();
	});

	it('applies the rate-limit wait when a 429 follows plain failures', () => {
		expect(planReadRetry([plain, plain, limited(20)])).toBe(20_000);
	});

	it('keeps the plain schedule for plain failures after a rate limit', () => {
		// The one rate-limited wait has been served; a plain failure after it
		// is the second plain failure of this read.
		expect(planReadRetry([plain, limited(20), plain])).toBe(2000);
	});
});

describe('planReadRetry properties', () => {
	it('plans the legacy schedule for any run of plain failures', () => {
		fc.assert(
			fc.property(fc.integer({ min: 1, max: 12 }), (n) => {
				const got = planReadRetry(Array<ReadFailure>(n).fill(plain));
				expect(got).toBe(n <= 4 ? 1000 * 2 ** (n - 1) : null);
			})
		);
	});

	it('waits between one second and one minute on a first rate limit, honouring the wait inside that band', () => {
		fc.assert(
			fc.property(
				fc.option(fc.integer({ min: 0, max: 100_000 }), { nil: null }),
				fc.integer({ min: 0, max: 3 }),
				(secs, plainBefore) => {
					const history = [...Array<ReadFailure>(plainBefore).fill(plain), limited(secs)];
					const got = planReadRetry(history);
					expect(got).not.toBeNull();
					expect(got!).toBeGreaterThanOrEqual(1_000);
					expect(got!).toBeLessThanOrEqual(60_000);
					if (secs !== null && secs >= 1 && secs <= 60) expect(got).toBe(secs * 1000);
				}
			)
		);
	});

	it('never plans another attempt after a second rate limit', () => {
		const failure = fc.oneof(
			fc.constant(plain),
			fc.option(fc.integer({ min: 0, max: 120 }), { nil: null }).map(limited)
		);
		fc.assert(
			fc.property(fc.array(failure, { minLength: 0, maxLength: 6 }), (history) => {
				const limits = history.filter((f) => f.rateLimited).length;
				const got = planReadRetry([...history, limited(30)]);
				if (limits >= 1) expect(got).toBeNull();
				else expect(got).toBe(30_000);
			})
		);
	});
});

// A permanent answer is not retried. The backend answers a tracker's
// rejection (a 4xx: `error.account.tracker_rejected`, HTTP 422) and a
// refused bearer (401) with statuses that say asking again gets the same
// answer; only a failure that can pass — a 5xx, the network, a timeout —
// is worth the schedule, besides the rate limit's own plan.
describe('a permanent failure ends the read', () => {
	const rejected = (status: number) =>
		new AccountApiError(
			status,
			wire<AniErrorPayload>({
				kind: 'tracker_rejected',
				status: 403,
				key: 'error.account.tracker_rejected'
			})
		);

	it('reads a 4xx other than a rate limit or a timeout as permanent', () => {
		for (const status of [400, 401, 403, 404, 410, 422]) {
			expect(classifyReadFailure(rejected(status)), String(status)).toMatchObject({
				permanent: true
			});
		}
	});

	it('reads a 5xx, a timeout, a transport failure and a missing bearer as passing', () => {
		for (const e of [
			rejected(502),
			rejected(503),
			rejected(504),
			rejected(408),
			new TypeError('fetch failed'),
			new Error('no bearer for mal')
		]) {
			expect(classifyReadFailure(e), String(e)).not.toMatchObject({ permanent: true });
		}
	});

	it('plans no retry after a permanent failure, wherever it falls', () => {
		const permanent = classifyReadFailure(rejected(422));
		expect(planReadRetry([permanent])).toBeNull();
		expect(planReadRetry([plain, plain, permanent])).toBeNull();
	});

	it('still retries a 5xx on the schedule', () => {
		expect(planReadRetry([classifyReadFailure(rejected(502))])).toBe(1000);
	});

	it('ends on any 4xx but 408 and 429, and retries any 5xx', () => {
		fc.assert(
			fc.property(fc.integer({ min: 400, max: 599 }), (status) => {
				const plan = planReadRetry([classifyReadFailure(rejected(status))]);
				if (status === 429) return; // the rate limit's own plan
				if (status >= 500 || status === 408) expect(plan).toBe(1000);
				else expect(plan).toBeNull();
			})
		);
	});
});
