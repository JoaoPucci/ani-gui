import { describe, expect, it } from 'vitest';
import { nextReadRetryMs } from './read-retry';

describe('nextReadRetryMs', () => {
	it('backs off exponentially from 1s', () => {
		expect(nextReadRetryMs(0)).toBe(1000);
		expect(nextReadRetryMs(1)).toBe(2000);
		expect(nextReadRetryMs(2)).toBe(4000);
		expect(nextReadRetryMs(3)).toBe(8000);
	});

	it('returns null once retries are exhausted (so the caller stops and shows an error)', () => {
		expect(nextReadRetryMs(4)).toBeNull();
		expect(nextReadRetryMs(9)).toBeNull();
	});
});
