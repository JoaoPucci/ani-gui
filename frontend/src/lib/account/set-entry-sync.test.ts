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
import { toastStore } from '$lib/toasts/store.svelte';
import { m } from '$lib/paraglide/messages';

/** The backend holding a write back: 429 with the wait, by shape (the
 *  api module is mocked here). */
const refused = (secs: number) =>
	Object.assign(new Error('rate limited'), {
		status: 429,
		detail: JSON.stringify({ kind: 'rate_limited', retry_after_secs: secs })
	});

const busyNotice = () =>
	m.account_busy_saving({ provider: m.account_provider_anilist(), seconds: 0 });

beforeEach(() => {
	getEntry.mockClear();
	getEntry.mockResolvedValue({ status: 'watching', progress: 3 });
	setEntry.mockReset();
	setEntry.mockResolvedValue({ progress_episodes: 3 });
	removeEntry.mockReset();
	removeEntry.mockResolvedValue(undefined);
	for (const item of [...toastStore.items]) toastStore.dismiss(item.id);
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
			statusChanged: false,
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
			await syncSetEntry('kitsu-12', { status: 'planning', statusChanged: false, progress: 0 })
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

describe('a save the tracker holds back', () => {
	it('waits it out with a notice, re-reads, and lands', async () => {
		byProvider.anilist = { kind: 'connected', account: { access_token: 'tok-a' } };
		setEntry.mockRejectedValueOnce(refused(0));
		const out = await syncSetEntry('kitsu-12', {
			status: 'dropped',
			statusChanged: true,
			progress: 3
		});
		expect(out).toEqual({ written: 1, failed: 0 });
		expect(setEntry).toHaveBeenCalledTimes(2);
		// The retry decides off a fresh read of the tracker's row.
		expect(getEntry).toHaveBeenCalledTimes(2);
		expect(toastStore.items.map((t) => t.message)).toContain(busyNotice());
	});

	it('waits out a removal the same way', async () => {
		byProvider.anilist = { kind: 'connected', account: { access_token: 'tok-a' } };
		removeEntry.mockRejectedValueOnce(refused(0));
		const out = await syncRemoveEntry('kitsu-12');
		expect(out).toEqual({ removed: 1, failed: 0 });
		expect(removeEntry).toHaveBeenCalledTimes(2);
		expect(toastStore.items.map((t) => t.message)).toContain(busyNotice());
	});
});
