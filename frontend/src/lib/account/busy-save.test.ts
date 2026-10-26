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
	it('stays up for the wait, and no shorter than a plain toast', () => {
		noticeBusy('anilist', 40);
		noticeBusy('mal', 1);
		const [long, short] = toastStore.items;
		expect(long).toMatchObject({
			kind: 'info',
			message: m.account_busy_saving({ provider: m.account_provider_anilist(), seconds: 40 }),
			duration: 40_000
		});
		expect(short.duration).toBe(4000);
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
