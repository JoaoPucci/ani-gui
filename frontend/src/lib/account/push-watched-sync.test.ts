import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

// Mock the live-store collaborators so the glue under test is the only
// real code: syncWatchedToTrackers must read the connected providers
// off the store, resolve each bearer via fresh-bearer, and delegate to
// the (already-tested) pure fan-out with the api updateProgress.
// `vi.hoisted` so the shared handles exist before the hoisted vi.mock
// factories run. The accounts here carry no refresh_token + a far-future
// expiry, so fresh-bearer's proactive-refresh branch never fires — it
// just returns the connected bearer. (refreshTokens/persistAccount are
// stubbed only because fresh-bearer imports them; the coalescing +
// refresh branches are unit-tested in fresh-bearer.test.ts.)
const { updateProgress, byProvider } = vi.hoisted(() => ({
	updateProgress: vi.fn(async () => null),
	byProvider: {} as Record<
		string,
		{
			kind: string;
			account?: {
				access_token: string;
				refresh_token?: string | null;
				expires_at_epoch_s?: number;
				user_id?: string;
			};
		}
	>
}));
vi.mock('./api', () => ({
	updateProgress,
	refreshTokens: vi.fn(),
	persistAccount: vi.fn(async () => ({ ok: true }))
}));
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

import { syncWatchedToTrackers } from './push-watched';
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

beforeEach(() => {
	updateProgress.mockReset();
	updateProgress.mockResolvedValue(null);
	for (const item of [...toastStore.items]) toastStore.dismiss(item.id);
	for (const k of Object.keys(byProvider)) delete byProvider[k];
});

describe('syncWatchedToTrackers', () => {
	it('fans the just-watched episode out to every connected tracker', async () => {
		byProvider.anilist = { kind: 'connected', account: { access_token: 'tok-a' } };
		byProvider.mal = { kind: 'connected', account: { access_token: 'tok-m' } };
		byProvider.inhouse = { kind: 'disconnected' };
		await syncWatchedToTrackers('kitsu-12', 7);
		expect(updateProgress).toHaveBeenCalledTimes(2);
		// Progress-only: a normal advance never sends a status override.
		expect(updateProgress).toHaveBeenCalledWith('anilist', 'tok-a', {
			kitsu_id: 'kitsu-12',
			progress: 7
		});
		expect(updateProgress).toHaveBeenCalledWith('mal', 'tok-m', {
			kitsu_id: 'kitsu-12',
			progress: 7
		});
	});

	it('no-ops when no provider is connected', async () => {
		await syncWatchedToTrackers('kitsu-12', 1);
		expect(updateProgress).not.toHaveBeenCalled();
	});

	it('holds a progress write AniList refuses, says so, and sends it when the wait ends', async () => {
		byProvider.anilist = { kind: 'connected', account: { access_token: 'tok-a' } };
		updateProgress.mockRejectedValueOnce(refused(0)).mockResolvedValueOnce(null);
		await syncWatchedToTrackers('kitsu-12', 7);
		expect(updateProgress).toHaveBeenCalledTimes(2);
		expect(toastStore.items.map((t) => t.message)).toContain(
			m.account_busy_saving({ provider: m.account_provider_anilist(), seconds: 0 })
		);
	});

	it('tells the user a progress write that could not be saved', async () => {
		byProvider.mal = { kind: 'connected', account: { access_token: 'tok-m' } };
		updateProgress.mockRejectedValue(new Error('offline'));
		await syncWatchedToTrackers('kitsu-12', 7);
		const errors = toastStore.items.filter((t) => t.kind === 'error').map((t) => t.message);
		expect(errors).toEqual([
			m.account_progress_save_failed({ provider: m.account_provider_mal() })
		]);
	});
});

describe('a held progress write a newer one replaces', () => {
	afterEach(() => {
		vi.useRealTimers();
	});

	it('is not re-sent and is not reported as a failure', async () => {
		vi.useFakeTimers();
		byProvider.anilist = { kind: 'connected', account: { access_token: 'tok-a' } };
		updateProgress.mockRejectedValueOnce(refused(1)).mockResolvedValue(null);
		const six = syncWatchedToTrackers('kitsu-12', 6);
		await vi.advanceTimersByTimeAsync(0);
		await syncWatchedToTrackers('kitsu-12', 4);
		await vi.advanceTimersByTimeAsync(1000);
		await six;
		const sent = updateProgress.mock.calls.map((c) => (c as unknown[])[2]);
		expect(sent).toEqual([
			{ kitsu_id: 'kitsu-12', progress: 6 },
			{ kitsu_id: 'kitsu-12', progress: 4 }
		]);
		expect(toastStore.items.filter((t) => t.kind === 'error')).toEqual([]);
	});
});

describe('a held finale a newer progress write covers', () => {
	afterEach(() => {
		vi.useRealTimers();
	});

	it('is dropped whole: its completion does not travel without its episode', async () => {
		// The backend only lets `completed` through beside the episode it
		// checks against the tracker's own total; a completion re-sent alone
		// would skip that check.
		vi.useFakeTimers();
		byProvider.anilist = { kind: 'connected', account: { access_token: 'tok-a' } };
		updateProgress.mockRejectedValueOnce(refused(1)).mockResolvedValue(null);
		const finale = syncWatchedToTrackers('kitsu-50', 12, 12, true);
		await vi.advanceTimersByTimeAsync(0);
		await syncWatchedToTrackers('kitsu-50', 11);
		await vi.advanceTimersByTimeAsync(1000);
		await finale;
		const sent = updateProgress.mock.calls.map((c) => (c as unknown[])[2]);
		expect(sent).toEqual([
			{ kitsu_id: 'kitsu-50', progress: 12, status: 'completed' },
			{ kitsu_id: 'kitsu-50', progress: 11 }
		]);
		expect(toastStore.items.filter((t) => t.kind === 'error')).toEqual([]);
	});

	it('keeps its episode when a newer write covered only the status', async () => {
		vi.useFakeTimers();
		byProvider.anilist = { kind: 'connected', account: { access_token: 'tok-a' } };
		updateProgress.mockRejectedValueOnce(refused(1)).mockResolvedValue(null);
		const finale = syncWatchedToTrackers('kitsu-51', 12, 12, true);
		await vi.advanceTimersByTimeAsync(0);
		await saveWhenTrackerAllows('anilist', 'kitsu-51', async () => null, ['status']);
		await vi.advanceTimersByTimeAsync(1000);
		await finale;
		expect(updateProgress).toHaveBeenLastCalledWith('anilist', 'tok-a', {
			kitsu_id: 'kitsu-51',
			progress: 12
		});
	});
});

describe('a held progress write whose tracker account changes during the wait', () => {
	afterEach(() => {
		vi.useRealTimers();
		accountStore.accountGeneration.anilist = 0;
	});

	it('is not sent to the account now connected, and is not reported', async () => {
		vi.useFakeTimers();
		byProvider.anilist = { kind: 'connected', account: { access_token: 'tok-a', user_id: 'u-a' } };
		updateProgress.mockRejectedValueOnce(refused(1)).mockResolvedValue(null);
		const write = syncWatchedToTrackers('kitsu-50', 6);
		await vi.advanceTimersByTimeAsync(0);
		accountStore.accountGeneration.anilist += 1;
		byProvider.anilist = { kind: 'connected', account: { access_token: 'tok-b', user_id: 'u-b' } };
		await vi.advanceTimersByTimeAsync(1000);
		await write;
		expect(updateProgress).toHaveBeenCalledTimes(1);
		expect(toastStore.items.filter((t) => t.kind === 'error')).toEqual([]);
	});

	it('re-sends with the bearer the same account holds by then', async () => {
		vi.useFakeTimers();
		byProvider.anilist = { kind: 'connected', account: { access_token: 'tok-a', user_id: 'u-a' } };
		updateProgress.mockRejectedValueOnce(refused(1)).mockResolvedValue(null);
		const write = syncWatchedToTrackers('kitsu-51', 6);
		await vi.advanceTimersByTimeAsync(0);
		accountStore.accountGeneration.anilist += 1;
		byProvider.anilist = { kind: 'connected', account: { access_token: 'tok-a2', user_id: 'u-a' } };
		await vi.advanceTimersByTimeAsync(1000);
		await write;
		expect(updateProgress).toHaveBeenLastCalledWith('anilist', 'tok-a2', expect.anything());
	});
});
