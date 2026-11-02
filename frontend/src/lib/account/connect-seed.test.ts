import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const { fetchAndCacheList } = vi.hoisted(() => ({
	fetchAndCacheList: vi.fn(async (): Promise<unknown[]> => [])
}));
vi.mock('./api', () => ({ fetchAndCacheList }));

import { seedConnectedList } from './connect-seed';
import { invalidateWatchLater, readLastRefreshed } from './watch-later-refresh';

function fakeStorage() {
	const m = new Map<string, string>();
	return {
		getItem: (k: string) => (m.has(k) ? (m.get(k) as string) : null),
		setItem: (k: string, v: string) => void m.set(k, String(v)),
		removeItem: (k: string) => void m.delete(k)
	};
}

beforeEach(() => {
	(globalThis as { window?: unknown }).window = { localStorage: fakeStorage() };
});
afterEach(() => {
	(globalThis as { window?: unknown }).window = undefined;
	fetchAndCacheList.mockReset();
});

describe('seedConnectedList', () => {
	it('pulls the list with the new bearer and stamps it fresh', async () => {
		fetchAndCacheList.mockResolvedValue([]);
		await seedConnectedList('mal', 'tok');
		expect(fetchAndCacheList).toHaveBeenCalledWith('mal', 'tok');
		expect(readLastRefreshed('mal')).not.toBeNull();
	});

	it('leaves the list unstamped when the pull fails, without throwing', async () => {
		fetchAndCacheList.mockRejectedValue(new Error('offline'));
		await expect(seedConnectedList('mal', 'tok')).resolves.toBeUndefined();
		expect(readLastRefreshed('mal')).toBeNull();
	});

	it('leaves the list due for a re-read when a write settles while the pull is out', async () => {
		fetchAndCacheList.mockImplementation(async () => {
			// A held write of the same account lands meanwhile.
			invalidateWatchLater('anilist');
			return [];
		});
		await seedConnectedList('anilist', 'tok');
		expect(readLastRefreshed('anilist')).toBeNull();
	});
});
