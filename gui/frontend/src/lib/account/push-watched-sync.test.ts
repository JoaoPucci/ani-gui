import { beforeEach, describe, expect, it, vi } from 'vitest';

// Mock the live-store collaborators so the glue under test is the only
// real code: syncWatchedToTrackers must read the connected providers
// off the store, resolve each bearer via state-helpers, and delegate
// to the (already-tested) pure fan-out with the api updateProgress.
// `vi.hoisted` so the shared handles exist before the hoisted vi.mock
// factories run.
const { updateProgress, byProvider } = vi.hoisted(() => ({
	updateProgress: vi.fn(async () => null),
	byProvider: {} as Record<string, { kind: string; account?: { access_token: string } }>
}));
vi.mock('./api', () => ({ updateProgress }));
vi.mock('./store.svelte', () => ({
	accountStore: {
		get connected() {
			return Object.keys(byProvider).filter((p) => byProvider[p].kind === 'connected');
		},
		get byProvider() {
			return byProvider;
		}
	}
}));

import { syncWatchedToTrackers } from './push-watched';

beforeEach(() => {
	updateProgress.mockClear();
	for (const k of Object.keys(byProvider)) delete byProvider[k];
});

describe('syncWatchedToTrackers', () => {
	it('fans the just-watched episode out to every connected tracker', async () => {
		byProvider.anilist = { kind: 'connected', account: { access_token: 'tok-a' } };
		byProvider.mal = { kind: 'connected', account: { access_token: 'tok-m' } };
		byProvider.inhouse = { kind: 'disconnected' };
		await syncWatchedToTrackers('kitsu-12', 7);
		expect(updateProgress).toHaveBeenCalledTimes(2);
		expect(updateProgress).toHaveBeenCalledWith('anilist', 'tok-a', {
			kitsu_id: 'kitsu-12',
			progress: 7,
			status: 'watching'
		});
		expect(updateProgress).toHaveBeenCalledWith('mal', 'tok-m', {
			kitsu_id: 'kitsu-12',
			progress: 7,
			status: 'watching'
		});
	});

	it('no-ops when no provider is connected', async () => {
		await syncWatchedToTrackers('kitsu-12', 1);
		expect(updateProgress).not.toHaveBeenCalled();
	});
});
