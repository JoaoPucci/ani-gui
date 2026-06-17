import { beforeEach, describe, expect, it, vi } from 'vitest';

// Mock the live-store collaborators so syncSetEntry / syncRemoveEntry are
// the only real code under test: read connected providers off the store,
// resolve each bearer via fresh-bearer, delegate to the (already-tested)
// pure fan-out, then invalidate the Watch Later snapshot per provider.
const { getEntry, setEntry, removeEntry, invalidateWatchLater, byProvider } = vi.hoisted(() => ({
	// Default getEntry → an existing entry, so the remove path (which only
	// counts trackers that had the row) and the set path both have a row to
	// act on. Individual tests override the resolved value as needed.
	getEntry: vi.fn(
		async (): Promise<{ status: string; progress: number } | null> => ({
			status: 'watching',
			progress: 3
		})
	),
	setEntry: vi.fn(async () => ({ progress_episodes: 3 })),
	removeEntry: vi.fn(async () => undefined),
	invalidateWatchLater: vi.fn(),
	byProvider: {} as Record<string, { kind: string; account?: { access_token: string } }>
}));
vi.mock('./entry-api', () => ({ getEntry, setEntry, removeEntry }));
vi.mock('./api', () => ({
	refreshTokens: vi.fn(),
	persistAccount: vi.fn(async () => ({ ok: true }))
}));
vi.mock('./watch-later-refresh', () => ({ invalidateWatchLater }));
vi.mock('./store.svelte', () => ({
	accountStore: {
		get connected() {
			return Object.keys(byProvider).filter((p) => byProvider[p].kind === 'connected');
		},
		get byProvider() {
			return byProvider;
		},
		accountGeneration: { anilist: 0, mal: 0, inhouse: 0 } as Record<string, number>,
		accountChanging: { anilist: false, mal: false, inhouse: false } as Record<string, boolean>,
		setConnected: vi.fn()
	}
}));

import { syncRemoveEntry, syncSetEntry } from './set-entry';

beforeEach(() => {
	getEntry.mockClear();
	getEntry.mockResolvedValue({ status: 'watching', progress: 3 });
	setEntry.mockClear();
	removeEntry.mockClear();
	invalidateWatchLater.mockClear();
	for (const k of Object.keys(byProvider)) delete byProvider[k];
});

describe('syncSetEntry', () => {
	it('fans the edit to every connected tracker and invalidates Watch Later', async () => {
		// Adding (no row on either tracker) → status is written to each.
		getEntry.mockResolvedValue(null);
		byProvider.anilist = { kind: 'connected', account: { access_token: 'tok-a' } };
		byProvider.mal = { kind: 'connected', account: { access_token: 'tok-m' } };
		const out = await syncSetEntry('kitsu-12', {
			status: 'watching',
			seededStatus: 'watching',
			progress: 3
		});
		expect(out).toEqual({ written: 2, failed: 0 });
		expect(setEntry).toHaveBeenCalledWith('anilist', 'tok-a', {
			kitsu_id: 'kitsu-12',
			status: 'watching',
			progress: 3
		});
		expect(invalidateWatchLater).toHaveBeenCalledTimes(2);
	});

	it('no-ops with no connected provider', async () => {
		expect(
			await syncSetEntry('kitsu-12', { status: 'planning', seededStatus: 'planning', progress: 0 })
		).toEqual({ written: 0, failed: 0 });
		expect(setEntry).not.toHaveBeenCalled();
	});
});

describe('syncRemoveEntry', () => {
	it('removes from every connected tracker and invalidates Watch Later', async () => {
		byProvider.anilist = { kind: 'connected', account: { access_token: 'tok-a' } };
		const out = await syncRemoveEntry('kitsu-12');
		expect(out).toEqual({ removed: 1, failed: 0 });
		expect(removeEntry).toHaveBeenCalledWith('anilist', 'tok-a', 'kitsu-12');
		expect(invalidateWatchLater).toHaveBeenCalledTimes(1);
	});
});
