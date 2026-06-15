import { describe, expect, it } from 'vitest';
import { isWatchLaterStale, WATCH_LATER_TTL_MS } from './watch-later-refresh';

describe('isWatchLaterStale', () => {
	const now = 1_000_000_000_000;

	it('treats a never-refreshed snapshot (null) as stale', () => {
		expect(isWatchLaterStale(null, now)).toBe(true);
	});

	it('is fresh within the TTL', () => {
		expect(isWatchLaterStale(now - (WATCH_LATER_TTL_MS - 1000), now)).toBe(false);
	});

	it('is stale once the TTL has elapsed', () => {
		expect(isWatchLaterStale(now - WATCH_LATER_TTL_MS, now)).toBe(true);
		expect(isWatchLaterStale(now - (WATCH_LATER_TTL_MS + 1000), now)).toBe(true);
	});

	it('honors a custom ttl', () => {
		expect(isWatchLaterStale(now - 5000, now, 10_000)).toBe(false);
		expect(isWatchLaterStale(now - 15_000, now, 10_000)).toBe(true);
	});
});
