import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import {
	noticeBusy,
	providerName,
	reportProgressNotSaved,
	saveWhenTrackerAllows,
	trackedWriteKeys
} from './busy-save';
import { SupersededError } from './save-when-allowed';
import { toastStore } from '$lib/toasts/store.svelte';
import { accountStore } from './store.svelte';
import { m } from '$lib/paraglide/messages';

beforeEach(() => {
	for (const item of [...toastStore.items]) toastStore.dismiss(item.id);
});

describe('providerName', () => {
	it('names each tracker by its display name, and an unnamed one by its id', () => {
		expect(providerName('anilist')).toBe(m.account_provider_anilist());
		expect(providerName('mal')).toBe(m.account_provider_mal());
		expect(providerName('inhouse')).toBe('inhouse');
	});
});

describe('noticeBusy', () => {
	afterEach(() => {
		vi.useRealTimers();
	});

	const busy = (seconds: number) =>
		m.account_busy_saving({ provider: m.account_provider_anilist(), seconds });
	const now = () => m.account_busy_saving_now({ provider: m.account_provider_anilist() });

	it('counts the wait down once a second, in place, and then says the save is going', async () => {
		// The countdown replaced the notice that sat on its first number for
		// the whole wait: it is pinned while the write is held, and its own
		// write takes it down (see the write's cases below).
		vi.useFakeTimers();
		const id = noticeBusy('anilist', 3);
		const row = () => toastStore.items.find((t) => t.id === id);
		expect(row()).toMatchObject({ kind: 'info', message: busy(3), duration: null });
		await vi.advanceTimersByTimeAsync(1000);
		expect(row()?.message).toBe(busy(2));
		await vi.advanceTimersByTimeAsync(1000);
		expect(row()?.message).toBe(busy(1));
		await vi.advanceTimersByTimeAsync(1000);
		expect(row()?.message).toBe(now());
		// Never a negative number, nor a frozen zero.
		await vi.advanceTimersByTimeAsync(5000);
		expect(row()?.message).toBe(now());
		expect(toastStore.items).toHaveLength(1);
	});

	it('reads the deadline, not a count of ticks: a late tick shows the true remainder', async () => {
		vi.useFakeTimers();
		const id = noticeBusy('anilist', 10);
		// The clock runs on four seconds without the timers firing — a page
		// in the background, a busy main thread — then one tick comes.
		vi.setSystemTime(Date.now() + 4000);
		await vi.advanceTimersByTimeAsync(1000);
		expect(toastStore.items.find((t) => t.id === id)?.message).toBe(busy(5));
	});

	it('stops its timer once the notice leaves the stack, whoever takes it down', async () => {
		vi.useFakeTimers();
		const before = vi.getTimerCount();
		const id = noticeBusy('anilist', 30);
		expect(vi.getTimerCount()).toBe(before + 1);
		toastStore.dismiss(id);
		expect(vi.getTimerCount()).toBe(before);
	});

	it('starts at "saving now" for a wait of nothing, with no timer to run', async () => {
		vi.useFakeTimers();
		const before = vi.getTimerCount();
		noticeBusy('anilist', 0);
		expect(toastStore.items.at(-1)?.message).toBe(now());
		expect(vi.getTimerCount()).toBe(before);
	});
});

describe('the notice of a held write', () => {
	beforeEach(() => {
		vi.useFakeTimers();
	});
	afterEach(() => {
		vi.useRealTimers();
	});

	const busyNotices = () => toastStore.items.filter((t) => t.kind === 'info');

	it('says the save is going when the wait ends, and goes once the write lands', async () => {
		let land: (v: string) => void = () => {};
		const write = vi
			.fn<() => Promise<string>>()
			.mockRejectedValueOnce(refused(2))
			.mockImplementationOnce(() => new Promise<string>((r) => (land = r)));
		const pending = saveWhenTrackerAllows('anilist', '80', write);
		await vi.advanceTimersByTimeAsync(0);
		expect(busyNotices().map((t) => t.message)).toEqual([
			m.account_busy_saving({ provider: m.account_provider_anilist(), seconds: 2 })
		]);
		await vi.advanceTimersByTimeAsync(2000);
		expect(busyNotices().map((t) => t.message)).toEqual([
			m.account_busy_saving_now({ provider: m.account_provider_anilist() })
		]);
		const timers = vi.getTimerCount();
		land('saved');
		expect(await pending).toBe('saved');
		expect(busyNotices()).toEqual([]);
		expect(vi.getTimerCount()).toBeLessThanOrEqual(timers);
	});

	it('goes once the write fails, its timer with it', async () => {
		const before = vi.getTimerCount();
		const write = vi
			.fn<() => Promise<string>>()
			.mockRejectedValueOnce(refused(3))
			.mockRejectedValueOnce(new Error('offline'));
		const pending = saveWhenTrackerAllows('anilist', '81', write).catch((e: unknown) => e);
		await vi.advanceTimersByTimeAsync(0);
		expect(busyNotices()).toHaveLength(1);
		await vi.advanceTimersByTimeAsync(3000);
		expect(await pending).toBeInstanceOf(Error);
		expect(busyNotices()).toEqual([]);
		expect(vi.getTimerCount()).toBe(before);
	});
});

describe('reportProgressNotSaved', () => {
	it('says which tracker did not take the progress', () => {
		reportProgressNotSaved('anilist');
		expect(toastStore.items).toEqual([
			expect.objectContaining({
				kind: 'error',
				message: m.account_progress_save_failed({ provider: m.account_provider_anilist() })
			})
		]);
	});
});

/** The backend holding a write back for `secs`. */
const refused = (secs: number) =>
	Object.assign(new Error('rate limited'), {
		status: 429,
		detail: JSON.stringify({ retry_after_secs: secs })
	});

const busyNotices = () => toastStore.items.filter((t) => t.kind === 'info');

describe('a held write and a newer one for the same show', () => {
	beforeEach(() => {
		vi.useFakeTimers();
	});
	afterEach(() => {
		vi.useRealTimers();
	});

	it('drops the held write when the newer one was issued during its wait', async () => {
		// The write-back of episode 6 waits; the user corrects to 4 in the
		// editor meanwhile. The held 6 must not raise it back.
		const six = vi.fn<() => Promise<string>>().mockRejectedValueOnce(refused(1));
		const settledHeld = saveWhenTrackerAllows('anilist', '12', six).catch((e: unknown) => e);
		await vi.advanceTimersByTimeAsync(0);
		let releaseFour: (v: string) => void = () => {};
		const four = vi.fn(() => new Promise<string>((r) => (releaseFour = r)));
		const newer = saveWhenTrackerAllows('anilist', '12', four);
		await vi.advanceTimersByTimeAsync(1000);
		expect(await settledHeld).toBeInstanceOf(SupersededError);
		expect(six).toHaveBeenCalledTimes(1);
		releaseFour('four');
		expect(await newer).toBe('four');
	});

	it('drops the held write when the newer one landed during its wait', async () => {
		// The editor's save of 5 waits; a write-back of 6 lands meanwhile. A
		// verbatim re-send of 5 would take the show back down.
		const five = vi.fn<() => Promise<string>>().mockRejectedValueOnce(refused(1));
		const held = saveWhenTrackerAllows('anilist', '12', five).catch((e: unknown) => e);
		await vi.advanceTimersByTimeAsync(0);
		expect(await saveWhenTrackerAllows('anilist', '12', async () => 'six')).toBe('six');
		await vi.advanceTimersByTimeAsync(1000);
		expect(await held).toBeInstanceOf(SupersededError);
		expect(five).toHaveBeenCalledTimes(1);
		// The dropped write's notice goes with it.
		expect(busyNotices()).toEqual([]);
	});

	it('keeps the held write when the newer one is another show or another tracker', async () => {
		const held = vi
			.fn<() => Promise<string>>()
			.mockRejectedValueOnce(refused(1))
			.mockResolvedValueOnce('held');
		const pending = saveWhenTrackerAllows('anilist', '12', held);
		await vi.advanceTimersByTimeAsync(0);
		await saveWhenTrackerAllows('anilist', '13', async () => 'other show');
		await saveWhenTrackerAllows('mal', '12', async () => 'other tracker');
		await vi.advanceTimersByTimeAsync(1000);
		expect(await pending).toBe('held');
		expect(held).toHaveBeenCalledTimes(2);
	});

	it('replaces its own notice on a second wait instead of stacking it', async () => {
		const write = vi
			.fn<() => Promise<string>>()
			.mockRejectedValueOnce(refused(1))
			.mockRejectedValueOnce(refused(2))
			.mockResolvedValueOnce('saved');
		const pending = saveWhenTrackerAllows('anilist', '12', write);
		await vi.advanceTimersByTimeAsync(1000);
		expect(busyNotices().map((t) => t.message)).toEqual([
			m.account_busy_saving({ provider: m.account_provider_anilist(), seconds: 2 })
		]);
		await vi.advanceTimersByTimeAsync(2000);
		expect(await pending).toBe('saved');
	});
});

describe('a held write and a newer one that covers part of it', () => {
	beforeEach(() => {
		vi.useFakeTimers();
	});
	afterEach(() => {
		vi.useRealTimers();
	});

	it('keeps the fields the newer write left alone, and only those', async () => {
		// A held save of status and progress; a progress-only write lands
		// during the wait. The status is still the held save's to send.
		const seen: (readonly string[])[] = [];
		const held = vi.fn(async (h: { left: () => readonly string[] }) => {
			seen.push(h.left());
			if (seen.length === 1) throw refused(1);
			return 'status sent';
		});
		const pending = saveWhenTrackerAllows('anilist', '21', held, ['status', 'progress']);
		await vi.advanceTimersByTimeAsync(0);
		await saveWhenTrackerAllows('anilist', '21', async () => 'progress', ['progress']);
		await vi.advanceTimersByTimeAsync(1000);
		expect(await pending).toBe('status sent');
		expect(seen).toEqual([['status', 'progress'], ['status']]);
	});

	it('is superseded once newer writes have covered every field it had', async () => {
		const held = vi.fn<() => Promise<string>>().mockRejectedValueOnce(refused(1));
		const pending = saveWhenTrackerAllows('anilist', '22', held, ['status', 'progress']).catch(
			(e: unknown) => e
		);
		await vi.advanceTimersByTimeAsync(0);
		await saveWhenTrackerAllows('anilist', '22', async () => 'p', ['progress']);
		await saveWhenTrackerAllows('anilist', '22', async () => 's', ['status', 'progress']);
		await vi.advanceTimersByTimeAsync(1000);
		expect(await pending).toBeInstanceOf(SupersededError);
		expect(held).toHaveBeenCalledTimes(1);
	});

	it("takes the older write's notice down as soon as a newer write starts", async () => {
		const held = vi
			.fn<() => Promise<string>>()
			.mockRejectedValueOnce(refused(5))
			.mockResolvedValue('held');
		const pending = saveWhenTrackerAllows('anilist', '23', held, ['status', 'progress']);
		await vi.advanceTimersByTimeAsync(0);
		expect(busyNotices()).toHaveLength(1);
		await saveWhenTrackerAllows('anilist', '23', async () => 'p', ['progress']);
		expect(busyNotices()).toEqual([]);
		await vi.advanceTimersByTimeAsync(5000);
		expect(await pending).toBe('held');
	});
});

describe('the write ledger', () => {
	beforeEach(() => {
		vi.useFakeTimers();
	});
	afterEach(() => {
		vi.useRealTimers();
	});

	it('forgets a show once its last write in flight settles, and not before', async () => {
		const before = trackedWriteKeys();
		const held = vi
			.fn<() => Promise<string>>()
			.mockRejectedValueOnce(refused(1))
			.mockResolvedValue('held');
		const pending = saveWhenTrackerAllows('anilist', '60', held, ['status', 'progress']);
		await vi.advanceTimersByTimeAsync(0);
		// A newer progress-only write settles while the older one still waits:
		// the older one's status is still its own, so the show is kept.
		await saveWhenTrackerAllows('anilist', '60', async () => 'p', ['progress']);
		expect(trackedWriteKeys()).toBe(before + 1);
		await vi.advanceTimersByTimeAsync(1000);
		expect(await pending).toBe('held');
		expect(trackedWriteKeys()).toBe(before);
		await saveWhenTrackerAllows('anilist', '61', async () => 'x').catch(() => {});
		expect(trackedWriteKeys()).toBe(before);
	});
});

describe('a held write dropped while it waits', () => {
	const as = (user_id: string) => ({
		access_token: `tok-${user_id}`,
		refresh_token: null,
		expires_at_epoch_s: 4_000_000_000,
		user_id,
		username: 'u',
		avatar_url: null
	});
	beforeEach(() => {
		vi.useFakeTimers();
		accountStore.setConnected('anilist', as('u1'));
	});
	afterEach(() => {
		accountStore.setDisconnected('anilist');
		vi.useRealTimers();
	});

	it('ends its wait and takes its notice down within a second of its account changing', async () => {
		const write = vi.fn<() => Promise<string>>().mockRejectedValueOnce(refused(60));
		const pending = saveWhenTrackerAllows('anilist', '90', write, undefined, { user: 'u1' }).catch(
			(e: unknown) => e
		);
		await vi.advanceTimersByTimeAsync(0);
		expect(toastStore.items.filter((t) => t.kind === 'info')).toHaveLength(1);

		accountStore.setConnected('anilist', as('u2'));
		await vi.advanceTimersByTimeAsync(1000);
		expect(toastStore.items.filter((t) => t.kind === 'info')).toEqual([]);
		expect(await pending).toBeInstanceOf(SupersededError);
		expect(write).toHaveBeenCalledTimes(1);
		expect(vi.getTimerCount()).toBe(0);
	});
});

describe('what the notice announces', () => {
	afterEach(() => {
		vi.useRealTimers();
	});

	it('announces its first words, not each second, and then that the save is going', async () => {
		vi.useFakeTimers();
		const id = noticeBusy('anilist', 3);
		const row = () => toastStore.items.find((t) => t.id === id)!;
		const first = m.account_busy_saving({ provider: m.account_provider_anilist(), seconds: 3 });
		expect(row().announcement).toBe(first);
		await vi.advanceTimersByTimeAsync(2000);
		expect(row().message).toBe(
			m.account_busy_saving({ provider: m.account_provider_anilist(), seconds: 1 })
		);
		expect(row().announcement).toBe(first);
		await vi.advanceTimersByTimeAsync(1000);
		const going = m.account_busy_saving_now({ provider: m.account_provider_anilist() });
		expect(row().message).toBe(going);
		expect(row().announcement).toBe(going);
		toastStore.dismiss(id);
	});
});
