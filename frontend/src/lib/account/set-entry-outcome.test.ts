import { describe, expect, it } from 'vitest';
import { isRateLimit, tallyFanout } from './set-entry-outcome';
import { AccountApiError } from './api';

describe('isRateLimit', () => {
	it('true for a 429 AccountApiError', () => {
		expect(isRateLimit(new AccountApiError(429, 'rate limited'))).toBe(true);
	});

	it('false for a non-429 AccountApiError', () => {
		expect(isRateLimit(new AccountApiError(502, 'bad gateway'))).toBe(false);
	});

	it('false for a plain Error (or anything else)', () => {
		expect(isRateLimit(new Error('boom'))).toBe(false);
		expect(isRateLimit('429')).toBe(false);
		expect(isRateLimit(null)).toBe(false);
	});
});

describe('tallyFanout', () => {
	it('counts ok + failed and leaves rateLimited false without a 429', () => {
		expect(tallyFanout(['ok', 'neither', 'failed'])).toEqual({
			ok: 1,
			failed: 1,
			rateLimited: false
		});
	});

	it('counts a 429 toward failed and flags rateLimited', () => {
		expect(tallyFanout(['ok', 'ratelimited'])).toEqual({ ok: 1, failed: 1, rateLimited: true });
	});

	it('empty → all zero, not rate-limited', () => {
		expect(tallyFanout([])).toEqual({ ok: 0, failed: 0, rateLimited: false });
	});
});
