import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

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
	byProvider: {} as Record<
		string,
		{ kind: string; account?: { access_token: string; user_id?: string } }
	>
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
import { saveWhenTrackerAllows } from './busy-save';
import { toastStore } from '$lib/toasts/store.svelte';
import { accountStore } from './store.svelte';
import { m } from '$lib/paraglide/messages';

/** The backend holding a write back: 429 with the wait, by shape (the
 *  api module is mocked here). */
const refused = (secs: number) =>
	Object.assign(new Error('rate limited'), {
		status: 429,
		detail: JSON.stringify({ kind: 'rate_limited', retry_after_secs: secs })
	});

/** A 0 s wait: the notice says the save is going now. */
const busyNotice = () => m.account_busy_saving_now({ provider: m.account_provider_anilist() });

/** The messages pushed to the toast stack while `run` goes. */
async function noticesDuring<T>(run: () => Promise<T>): Promise<{ out: T; pushed: string[] }> {
	const push = vi.spyOn(toastStore, 'push');
	try {
		const out = await run();
		return { out, pushed: push.mock.calls.map(([args]) => args.message) };
	} finally {
		push.mockRestore();
	}
}

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
		const { out, pushed } = await noticesDuring(() =>
			syncSetEntry('kitsu-12', { status: 'dropped', statusChanged: true, progress: 3 })
		);
		expect(out).toEqual({ written: 1, failed: 0 });
		expect(setEntry).toHaveBeenCalledTimes(2);
		// The retry decides off a fresh read of the tracker's row.
		expect(getEntry).toHaveBeenCalledTimes(2);
		expect(pushed).toContain(busyNotice());
		// Landed: the notice is down.
		expect(toastStore.items.map((t) => t.message)).not.toContain(busyNotice());
	});

	it('waits out a removal the same way', async () => {
		byProvider.anilist = { kind: 'connected', account: { access_token: 'tok-a' } };
		removeEntry.mockRejectedValueOnce(refused(0));
		const { out, pushed } = await noticesDuring(() => syncRemoveEntry('kitsu-12'));
		expect(out).toEqual({ removed: 1, failed: 0 });
		expect(removeEntry).toHaveBeenCalledTimes(2);
		expect(pushed).toContain(busyNotice());
		expect(toastStore.items.map((t) => t.message)).not.toContain(busyNotice());
	});
});

describe('a save the tracker keeps refusing', () => {
	it('fails rate-limited once every attempt was refused', async () => {
		byProvider.anilist = { kind: 'connected', account: { access_token: 'tok-a' } };
		setEntry.mockRejectedValue(refused(0));
		const out = await syncSetEntry('kitsu-12', {
			status: 'dropped',
			statusChanged: true,
			progress: 3
		});
		expect(out).toEqual({ written: 0, failed: 1, rateLimited: true });
		expect(setEntry).toHaveBeenCalledTimes(4);
	});

	it('fails without another attempt on a failure that is not a rate limit', async () => {
		byProvider.anilist = { kind: 'connected', account: { access_token: 'tok-a' } };
		setEntry.mockRejectedValueOnce(refused(0)).mockRejectedValueOnce(new Error('offline'));
		const out = await syncSetEntry('kitsu-12', {
			status: 'dropped',
			statusChanged: true,
			progress: 3
		});
		expect(out).toEqual({ written: 0, failed: 1 });
		expect(setEntry).toHaveBeenCalledTimes(2);
	});
});

describe('a held save a newer write replaces', () => {
	afterEach(() => {
		vi.useRealTimers();
	});

	it('is neither written nor failed, and says it was superseded', async () => {
		vi.useFakeTimers();
		byProvider.anilist = { kind: 'connected', account: { access_token: 'tok-a' } };
		setEntry.mockRejectedValueOnce(refused(1));
		const held = syncSetEntry('kitsu-12', { status: 'dropped', statusChanged: true, progress: 5 });
		await vi.advanceTimersByTimeAsync(0);
		await syncSetEntry('kitsu-12', { status: 'watching', statusChanged: true, progress: 6 });
		await vi.advanceTimersByTimeAsync(1000);
		expect(await held).toEqual({ written: 0, failed: 0, superseded: true });
		expect(setEntry).toHaveBeenCalledTimes(2);
	});
});

describe('a held save a newer write covers in part', () => {
	afterEach(() => {
		vi.useRealTimers();
	});

	it('re-sends only the status when a progress-only write landed during the wait', async () => {
		vi.useFakeTimers();
		byProvider.anilist = { kind: 'connected', account: { access_token: 'tok-a' } };
		setEntry.mockRejectedValueOnce(refused(1));
		const held = syncSetEntry('kitsu-30', { status: 'paused', statusChanged: true, progress: 5 });
		await vi.advanceTimersByTimeAsync(0);
		// The progress write-back of a watched episode, for the same show.
		await saveWhenTrackerAllows('anilist', 'kitsu-30', async () => null, ['progress']);
		await vi.advanceTimersByTimeAsync(1000);
		expect(await held).toEqual({ written: 1, failed: 0 });
		expect(setEntry).toHaveBeenLastCalledWith('anilist', 'tok-a', {
			kitsu_id: 'kitsu-30',
			status: 'paused'
		});
	});

	it('re-sends only the status when a later save left the status alone', async () => {
		vi.useFakeTimers();
		byProvider.anilist = { kind: 'connected', account: { access_token: 'tok-a' } };
		setEntry.mockRejectedValueOnce(refused(1));
		const held = syncSetEntry('kitsu-31', { status: 'dropped', statusChanged: true, progress: 5 });
		await vi.advanceTimersByTimeAsync(0);
		await syncSetEntry('kitsu-31', { status: 'watching', statusChanged: false, progress: 6 });
		await vi.advanceTimersByTimeAsync(1000);
		await held;
		expect(setEntry).toHaveBeenLastCalledWith('anilist', 'tok-a', {
			kitsu_id: 'kitsu-31',
			status: 'dropped'
		});
	});

	it('is not sent when a newer write starts between its re-read and its write', async () => {
		vi.useFakeTimers();
		byProvider.anilist = { kind: 'connected', account: { access_token: 'tok-a' } };
		setEntry.mockRejectedValueOnce(refused(1));
		let reads = 0;
		getEntry.mockImplementation(async () => {
			reads += 1;
			// The re-sent attempt's read: a newer save of the whole entry
			// starts while it is out.
			if (reads === 2) {
				void saveWhenTrackerAllows('anilist', 'kitsu-32', async () => null, ['status', 'progress']);
			}
			return { status: 'watching', progress: 3 };
		});
		const held = syncSetEntry('kitsu-32', { status: 'dropped', statusChanged: true, progress: 5 });
		await vi.advanceTimersByTimeAsync(1000);
		expect(await held).toEqual({ written: 0, failed: 0, superseded: true });
		expect(setEntry).toHaveBeenCalledTimes(1);
	});
});

describe('a held removal a newer write covers in part', () => {
	afterEach(() => {
		vi.useRealTimers();
	});

	it('is superseded rather than sent, since it cannot go in part', async () => {
		vi.useFakeTimers();
		byProvider.anilist = { kind: 'connected', account: { access_token: 'tok-a' } };
		removeEntry.mockRejectedValueOnce(refused(1));
		const held = syncRemoveEntry('kitsu-40');
		await vi.advanceTimersByTimeAsync(0);
		await saveWhenTrackerAllows('anilist', 'kitsu-40', async () => null, ['progress']);
		await vi.advanceTimersByTimeAsync(1000);
		expect(await held).toEqual({ removed: 0, failed: 0, superseded: true });
		expect(removeEntry).toHaveBeenCalledTimes(1);
	});
});

describe('a held completion a newer write covers the episode of', () => {
	afterEach(() => {
		vi.useRealTimers();
	});

	it('is dropped whole rather than re-sent without its episode', async () => {
		vi.useFakeTimers();
		byProvider.anilist = { kind: 'connected', account: { access_token: 'tok-a' } };
		setEntry.mockRejectedValueOnce(refused(1));
		const held = syncSetEntry('kitsu-70', {
			status: 'completed',
			statusChanged: true,
			progress: 12,
			total: 12
		});
		await vi.advanceTimersByTimeAsync(0);
		await saveWhenTrackerAllows('anilist', 'kitsu-70', async () => null, ['progress']);
		await vi.advanceTimersByTimeAsync(1000);
		expect(await held).toEqual({ written: 0, failed: 0, superseded: true });
		expect(setEntry).toHaveBeenCalledTimes(1);
	});
});

describe('a held write whose tracker account changes during the wait', () => {
	afterEach(() => {
		vi.useRealTimers();
		accountStore.accountGeneration.anilist = 0;
		accountStore.accountChanging.anilist = false;
	});

	const save = { status: 'dropped' as const, statusChanged: true, progress: 5 };

	it('is neither read nor sent again when the account is disconnected or switched', async () => {
		vi.useFakeTimers();
		byProvider.anilist = { kind: 'connected', account: { access_token: 'tok-a', user_id: 'u-a' } };
		setEntry.mockRejectedValueOnce(refused(1));
		const held = syncSetEntry('kitsu-40', save);
		await vi.advanceTimersByTimeAsync(0);
		// The user disconnects AniList and signs in as someone else.
		accountStore.accountGeneration.anilist += 1;
		byProvider.anilist = { kind: 'connected', account: { access_token: 'tok-b', user_id: 'u-b' } };
		await vi.advanceTimersByTimeAsync(1000);
		expect(await held).toEqual({ written: 0, failed: 0, superseded: true });
		expect(getEntry).toHaveBeenCalledTimes(1);
		expect(setEntry).toHaveBeenCalledTimes(1);
		expect(toastStore.items.filter((t) => t.kind === 'info')).toEqual([]);
	});

	it('is not sent while the account is being changed', async () => {
		vi.useFakeTimers();
		byProvider.anilist = { kind: 'connected', account: { access_token: 'tok-a', user_id: 'u-a' } };
		setEntry.mockRejectedValueOnce(refused(1));
		const held = syncSetEntry('kitsu-41', save);
		await vi.advanceTimersByTimeAsync(0);
		accountStore.accountChanging.anilist = true;
		await vi.advanceTimersByTimeAsync(1000);
		expect(await held).toEqual({ written: 0, failed: 0, superseded: true });
		expect(setEntry).toHaveBeenCalledTimes(1);
	});

	it('is not written when the account changes while its re-sent read is out', async () => {
		vi.useFakeTimers();
		byProvider.anilist = { kind: 'connected', account: { access_token: 'tok-a', user_id: 'u-a' } };
		setEntry.mockRejectedValueOnce(refused(1));
		let reads = 0;
		getEntry.mockImplementation(async () => {
			reads += 1;
			if (reads === 2) byProvider.anilist = { kind: 'disconnected' };
			return { status: 'watching', progress: 3 };
		});
		const held = syncSetEntry('kitsu-42', save);
		await vi.advanceTimersByTimeAsync(1000);
		expect(await held).toEqual({ written: 0, failed: 0, superseded: true });
		expect(setEntry).toHaveBeenCalledTimes(1);
	});

	it('re-sends with the bearer the same account holds by then', async () => {
		vi.useFakeTimers();
		byProvider.anilist = { kind: 'connected', account: { access_token: 'tok-a', user_id: 'u-a' } };
		setEntry.mockRejectedValueOnce(refused(1));
		const held = syncSetEntry('kitsu-43', save);
		await vi.advanceTimersByTimeAsync(0);
		// A refresh of the same account's token. The store's setConnected
		// moves the generation on for it, though the account is the same.
		accountStore.accountGeneration.anilist += 1;
		byProvider.anilist = { kind: 'connected', account: { access_token: 'tok-a2', user_id: 'u-a' } };
		await vi.advanceTimersByTimeAsync(1000);
		expect(await held).toEqual({ written: 1, failed: 0 });
		expect(setEntry).toHaveBeenLastCalledWith('anilist', 'tok-a2', expect.anything());
	});

	it('removes nothing once the account has changed', async () => {
		vi.useFakeTimers();
		byProvider.anilist = { kind: 'connected', account: { access_token: 'tok-a', user_id: 'u-a' } };
		removeEntry.mockRejectedValueOnce(refused(1));
		const held = syncRemoveEntry('kitsu-44');
		await vi.advanceTimersByTimeAsync(0);
		byProvider.anilist = { kind: 'disconnected' };
		await vi.advanceTimersByTimeAsync(1000);
		await held;
		expect(removeEntry).toHaveBeenCalledTimes(1);
	});
});

describe('a held write and the account around it, further', () => {
	afterEach(() => {
		vi.useRealTimers();
		accountStore.accountChanging.anilist = false;
	});

	const save = { status: 'dropped' as const, statusChanged: true, progress: 5 };

	it('is not sent at all when it starts while the account is being changed', async () => {
		// The save fires while a disconnect is under way: the tracker is
		// still listed, but the account it would be sent for is going.
		vi.useFakeTimers();
		byProvider.anilist = { kind: 'connected', account: { access_token: 'tok-a', user_id: 'u-a' } };
		accountStore.accountChanging.anilist = true;
		const held = syncSetEntry('kitsu-60', save);
		await vi.advanceTimersByTimeAsync(0);
		byProvider.anilist = { kind: 'disconnected' };
		accountStore.accountChanging.anilist = false;
		await vi.advanceTimersByTimeAsync(1000);
		expect(await held).toEqual({ written: 0, failed: 0, superseded: true });
		expect(getEntry).not.toHaveBeenCalled();
		expect(setEntry).not.toHaveBeenCalled();
	});

	it('is still sent after the account is marked expired, the same user behind it', async () => {
		// The account page's hydrate marks a past-expiry token expired
		// before its refresh restores it: the account has not changed.
		vi.useFakeTimers();
		byProvider.anilist = { kind: 'connected', account: { access_token: 'tok-a', user_id: 'u-a' } };
		setEntry.mockRejectedValueOnce(refused(1));
		const held = syncSetEntry('kitsu-61', save);
		await vi.advanceTimersByTimeAsync(0);
		byProvider.anilist = { kind: 'expired', account: { access_token: 'tok-a', user_id: 'u-a' } };
		await vi.advanceTimersByTimeAsync(1000);
		await held;
		expect(setEntry).toHaveBeenCalledTimes(2);
	});
});

describe('a write whose account goes before it starts', () => {
	it('is not sent when the disconnect completes while its bearer is resolved', async () => {
		// The fan-out has listed the tracker; the disconnect finishes in
		// the await for its bearer, which still hands back the old token.
		byProvider.anilist = { kind: 'connected', account: { access_token: 'tok-a', user_id: 'u-a' } };
		const held = syncSetEntry('kitsu-70', {
			status: 'dropped' as const,
			statusChanged: true,
			progress: 5
		});
		byProvider.anilist = { kind: 'disconnected' };
		expect(await held).toEqual({ written: 0, failed: 0, superseded: true });
		expect(getEntry).not.toHaveBeenCalled();
		expect(setEntry).not.toHaveBeenCalled();
	});

	it('removes nothing when the disconnect completes while its bearer is resolved', async () => {
		byProvider.anilist = { kind: 'connected', account: { access_token: 'tok-a', user_id: 'u-a' } };
		const held = syncRemoveEntry('kitsu-71');
		byProvider.anilist = { kind: 'disconnected' };
		await held;
		expect(getEntry).not.toHaveBeenCalled();
		expect(removeEntry).not.toHaveBeenCalled();
	});

	it('is not sent for another user who signed in while its bearer is resolved', async () => {
		byProvider.anilist = { kind: 'connected', account: { access_token: 'tok-a', user_id: 'u-a' } };
		const held = syncSetEntry('kitsu-72', {
			status: 'dropped' as const,
			statusChanged: true,
			progress: 5
		});
		byProvider.anilist = { kind: 'connected', account: { access_token: 'tok-b', user_id: 'u-b' } };
		await held;
		expect(setEntry).not.toHaveBeenCalled();
	});
});

describe('a write dropped mid-change still counts as the newer write', () => {
	afterEach(() => {
		vi.useRealTimers();
		accountStore.accountChanging.anilist = false;
	});

	it('keeps an older held write from re-sending what the dropped one set', async () => {
		// An older save waits; the user saves again while a disconnect is
		// under way, and that save is dropped. The disconnect then fails and
		// the account stays: the older save must not put back what the
		// user changed since.
		vi.useFakeTimers();
		byProvider.anilist = { kind: 'connected', account: { access_token: 'tok-a', user_id: 'u-a' } };
		setEntry.mockRejectedValueOnce(refused(1));
		const older = syncSetEntry('kitsu-80', {
			status: 'dropped' as const,
			statusChanged: true,
			progress: 5
		});
		await vi.advanceTimersByTimeAsync(0);
		accountStore.accountChanging.anilist = true;
		await syncSetEntry('kitsu-80', {
			status: 'watching' as const,
			statusChanged: true,
			progress: 6
		});
		accountStore.accountChanging.anilist = false;
		await vi.advanceTimersByTimeAsync(1000);
		expect(await older).toEqual({ written: 0, failed: 0, superseded: true });
		expect(setEntry).toHaveBeenCalledTimes(1);
	});
});
