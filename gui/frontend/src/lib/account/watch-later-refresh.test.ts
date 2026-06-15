import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import {
	invalidateWatchLater,
	isWatchLaterStale,
	markRefreshed,
	readLastRefreshed,
	WATCH_LATER_TTL_MS
} from './watch-later-refresh';

/** Minimal in-memory localStorage so the IO helpers run under the
 *  `node` test environment (mirrors store.svelte.test's window stub). */
function fakeStorage() {
	const m = new Map<string, string>();
	return {
		getItem: (k: string) => (m.has(k) ? (m.get(k) as string) : null),
		setItem: (k: string, v: string) => void m.set(k, String(v)),
		removeItem: (k: string) => void m.delete(k)
	};
}

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

describe('watch-later refresh timestamps (localStorage)', () => {
	beforeEach(() => {
		(globalThis as { window?: unknown }).window = { localStorage: fakeStorage() };
	});
	afterEach(() => {
		(globalThis as { window?: unknown }).window = undefined;
	});

	it('round-trips a mark/read per provider', () => {
		expect(readLastRefreshed('anilist')).toBeNull();
		markRefreshed('anilist', 1234);
		expect(readLastRefreshed('anilist')).toBe(1234);
		// Independent per provider.
		expect(readLastRefreshed('mal')).toBeNull();
	});

	it('invalidate clears the stamp so the snapshot reads stale again', () => {
		markRefreshed('mal', 9999);
		expect(readLastRefreshed('mal')).toBe(9999);
		invalidateWatchLater('mal');
		expect(readLastRefreshed('mal')).toBeNull();
		expect(isWatchLaterStale(readLastRefreshed('mal'), 9999)).toBe(true);
	});

	it('reads back null for a non-numeric stored value', () => {
		window.localStorage.setItem('aniGui:watchLater:lastRefresh:anilist', 'garbage');
		expect(readLastRefreshed('anilist')).toBeNull();
	});
});
