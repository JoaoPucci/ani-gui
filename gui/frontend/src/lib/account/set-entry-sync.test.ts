import { beforeEach, describe, expect, it, vi } from 'vitest';

// Mock the live-store collaborators so syncSetEntry / syncRemoveEntry are
// the only real code under test: read connected providers off the store,
// resolve each bearer via fresh-bearer, delegate to the (already-tested)
// pure fan-out, then invalidate the Watch Later snapshot per provider.
const { setEntry, removeEntry, invalidateWatchLater, byProvider } = vi.hoisted(() => ({
	setEntry: vi.fn(async () => ({ progress_episodes: 3 })),
	removeEntry: vi.fn(async () => undefined),
	invalidateWatchLater: vi.fn(),
	byProvider: {} as Record<string, { kind: string; account?: { access_token: string } }>
}));
vi.mock('./api', () => ({
	setEntry,
	removeEntry,
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
	setEntry.mockClear();
	removeEntry.mockClear();
	invalidateWatchLater.mockClear();
	for (const k of Object.keys(byProvider)) delete byProvider[k];
});

describe('syncSetEntry', () => {
	it('fans the edit to every connected tracker and invalidates Watch Later', async () => {
		byProvider.anilist = { kind: 'connected', account: { access_token: 'tok-a' } };
		byProvider.mal = { kind: 'connected', account: { access_token: 'tok-m' } };
		const n = await syncSetEntry('kitsu-12', { status: 'watching', progress: 3 });
		expect(n).toBe(2);
		expect(setEntry).toHaveBeenCalledWith('anilist', 'tok-a', {
			kitsu_id: 'kitsu-12',
			status: 'watching',
			progress: 3
		});
		expect(invalidateWatchLater).toHaveBeenCalledTimes(2);
	});

	it('no-ops with no connected provider', async () => {
		expect(await syncSetEntry('kitsu-12', { status: 'planning' })).toBe(0);
		expect(setEntry).not.toHaveBeenCalled();
	});
});

describe('syncRemoveEntry', () => {
	it('removes from every connected tracker and invalidates Watch Later', async () => {
		byProvider.anilist = { kind: 'connected', account: { access_token: 'tok-a' } };
		const n = await syncRemoveEntry('kitsu-12');
		expect(n).toBe(1);
		expect(removeEntry).toHaveBeenCalledWith('anilist', 'tok-a', 'kitsu-12');
		expect(invalidateWatchLater).toHaveBeenCalledTimes(1);
	});
});
