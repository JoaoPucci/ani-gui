import { describe, expect, it } from 'vitest';
import { AccountApiError } from './api';
import { classifyReadFailure, planReadRetry, type ReadFailure } from './read-retry';

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
});

describe('classifyReadFailure', () => {
	it('reads a 429 with the wait its envelope carries', () => {
		const e = new AccountApiError(
			429,
			JSON.stringify({
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
			JSON.stringify({
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
