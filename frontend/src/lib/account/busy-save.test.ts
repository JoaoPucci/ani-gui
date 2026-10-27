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
afterEach(() => {
	vi.restoreAllMocks();
});

/** Time passing by `ms` that no timer saw: the monotonic clock jumps on
 *  while every timer stays where it was, as when a hidden window's
 *  timers are throttled and wake late. */
function runClockAhead(ms: number): void {
	const now = performance.now.bind(performance);
	vi.spyOn(performance, 'now').mockImplementation(() => now() + ms);
}

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
		const id = noticeBusy('anilist', 3, 'anilist');
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
		const id = noticeBusy('anilist', 10, 'anilist');
		// Time runs on four seconds without the timers firing — a page in
		// the background, a busy main thread — then one tick comes.
		runClockAhead(4000);
		await vi.advanceTimersByTimeAsync(1000);
		expect(toastStore.items.find((t) => t.id === id)?.message).toBe(busy(5));
	});

	it('keeps its count through a wall-clock jump either way', async () => {
		vi.useFakeTimers();
		const id = noticeBusy('anilist', 10, 'anilist');
		vi.setSystemTime(Date.now() + 60_000);
		await vi.advanceTimersByTimeAsync(1000);
		expect(toastStore.items.find((t) => t.id === id)?.message).toBe(busy(9));
		vi.setSystemTime(Date.now() - 120_000);
		await vi.advanceTimersByTimeAsync(1000);
		expect(toastStore.items.find((t) => t.id === id)?.message).toBe(busy(8));
		toastStore.dismiss(id);
	});

	it('stops its timer once the notice leaves the stack, whoever takes it down', async () => {
		vi.useFakeTimers();
		const before = vi.getTimerCount();
		const id = noticeBusy('anilist', 30, 'anilist');
		expect(vi.getTimerCount()).toBe(before + 1);
		toastStore.dismiss(id);
		expect(vi.getTimerCount()).toBe(before);
	});

	it('stays on the stack, counting, however many passing toasts come after it', async () => {
		vi.useFakeTimers();
		const id = noticeBusy('anilist', 5, 'anilist');
		for (let i = 0; i < 5; i += 1) {
			toastStore.push({ kind: 'info', message: `passing ${i}`, duration: 9999 });
		}
		await vi.advanceTimersByTimeAsync(1000);
		expect(toastStore.items.find((t) => t.id === id)?.message).toBe(busy(4));
		toastStore.dismiss(id);
	});

	it('starts at "saving now" for a wait of nothing, with no timer to run', async () => {
		vi.useFakeTimers();
		const before = vi.getTimerCount();
		noticeBusy('anilist', 0, 'anilist');
		expect(toastStore.items.at(-1)?.message).toBe(now());
		expect(vi.getTimerCount()).toBe(before);
	});
});

describe('the notice of a write another tracker refused', () => {
	afterEach(() => {
		vi.useRealTimers();
	});
	const anilist = () => m.account_provider_anilist();
	const mal = () => m.account_provider_mal();

	it('names the tracker that is busy and the one the save is for, counting down', async () => {
		vi.useFakeTimers();
		const id = noticeBusy('mal', 3, 'anilist');
		const row = () => toastStore.items.find((t) => t.id === id);
		expect(row()?.message).toBe(
			m.account_busy_saving_for({ busy: anilist(), provider: mal(), seconds: 3 })
		);
		await vi.advanceTimersByTimeAsync(1000);
		expect(row()?.message).toBe(
			m.account_busy_saving_for({ busy: anilist(), provider: mal(), seconds: 2 })
		);
		await vi.advanceTimersByTimeAsync(2000);
		expect(row()?.message).toBe(m.account_busy_saving_now({ provider: mal() }));
		toastStore.dismiss(id);
	});

	it("says the save's own tracker is busy when that one refused", () => {
		const id = noticeBusy('mal', 4, 'mal');
		expect(toastStore.items.find((t) => t.id === id)?.message).toBe(
			m.account_busy_saving({ provider: mal(), seconds: 4 })
		);
		toastStore.dismiss(id);
	});

	it('names no busy tracker when the refusal named none, as a Kitsu 429 on the map read does', async () => {
		// Kitsu's 429 reaches the write as a 429 that names no tracker: the
		// write's own tracker did not refuse it, and the notice does not say
		// it did.
		vi.useFakeTimers();
		const write = vi
			.fn<() => Promise<string>>()
			.mockRejectedValueOnce(
				Object.assign(new Error('upstream'), {
					status: 429,
					detail: JSON.stringify({ kind: 'upstream', status: 429 })
				})
			)
			.mockResolvedValue('saved');
		const pending = saveWhenTrackerAllows('mal', '61', write);
		await vi.advanceTimersByTimeAsync(0);
		expect(toastStore.items.filter((t) => t.kind === 'info').map((t) => t.message)).toEqual([
			m.account_busy_saving_unnamed({ provider: mal(), seconds: 60 })
		]);
		await vi.advanceTimersByTimeAsync(60_000);
		expect(await pending).toBe('saved');
	});

	it('is what a MyAnimeList write AniList refused shows', async () => {
		vi.useFakeTimers();
		const write = vi
			.fn<() => Promise<string>>()
			.mockRejectedValueOnce(
				Object.assign(new Error('rate limited'), {
					status: 429,
					detail: JSON.stringify({ retry_after_secs: 5, refused_by: 'anilist' })
				})
			)
			.mockResolvedValue('saved');
		const pending = saveWhenTrackerAllows('mal', '60', write);
		await vi.advanceTimersByTimeAsync(0);
		expect(toastStore.items.filter((t) => t.kind === 'info').map((t) => t.message)).toEqual([
			m.account_busy_saving_for({ busy: anilist(), provider: mal(), seconds: 5 })
		]);
		await vi.advanceTimersByTimeAsync(5000);
		expect(await pending).toBe('saved');
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

/** AniList holding a write back for `secs`. */
const refused = (secs: number) =>
	Object.assign(new Error('rate limited'), {
		status: 429,
		detail: JSON.stringify({ retry_after_secs: secs, refused_by: 'anilist' })
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

	/** A write listed under u1, refused once with `secs` to wait, then taken. */
	function heldUnderU1(id: string, secs: number) {
		const write = vi
			.fn<() => Promise<string>>()
			.mockRejectedValueOnce(refused(secs))
			.mockResolvedValue('saved');
		const pending = saveWhenTrackerAllows('anilist', id, write, undefined, { user: 'u1' }).catch(
			(e: unknown) => e
		);
		return { write, pending };
	}

	it('keeps waiting through a reconnect, and goes when the same user is back', async () => {
		const { write, pending } = heldUnderU1('91', 5);
		await vi.advanceTimersByTimeAsync(0);
		accountStore.setConnecting('anilist');
		await vi.advanceTimersByTimeAsync(2000);
		expect(toastStore.items.filter((t) => t.kind === 'info')).toHaveLength(1);
		accountStore.setConnected('anilist', as('u1'));
		await vi.advanceTimersByTimeAsync(3000);
		expect(await pending).toBe('saved');
		expect(write).toHaveBeenCalledTimes(2);
	});

	it('keeps waiting through an account change that ends on the same user', async () => {
		const { write, pending } = heldUnderU1('92', 5);
		await vi.advanceTimersByTimeAsync(0);
		accountStore.beginAccountChange('anilist');
		await vi.advanceTimersByTimeAsync(2000);
		accountStore.setConnected('anilist', as('u1'));
		await vi.advanceTimersByTimeAsync(3000);
		expect(await pending).toBe('saved');
		expect(write).toHaveBeenCalledTimes(2);
	});

	it('is dropped within a second once a reconnect ends on another user', async () => {
		const { write, pending } = heldUnderU1('93', 60);
		await vi.advanceTimersByTimeAsync(0);
		accountStore.setConnecting('anilist');
		await vi.advanceTimersByTimeAsync(2000);
		accountStore.setConnected('anilist', as('u2'));
		await vi.advanceTimersByTimeAsync(1000);
		expect(await pending).toBeInstanceOf(SupersededError);
		expect(write).toHaveBeenCalledTimes(1);
	});

	it('is dropped at the end of its wait while the reconnect is still under way', async () => {
		const { write, pending } = heldUnderU1('94', 5);
		await vi.advanceTimersByTimeAsync(0);
		accountStore.setConnecting('anilist');
		await vi.advanceTimersByTimeAsync(5000);
		expect(await pending).toBeInstanceOf(SupersededError);
		expect(write).toHaveBeenCalledTimes(1);
	});

	it('is dropped within a second once the account is disconnected', async () => {
		const { write, pending } = heldUnderU1('95', 60);
		await vi.advanceTimersByTimeAsync(0);
		accountStore.setDisconnected('anilist');
		await vi.advanceTimersByTimeAsync(1000);
		expect(await pending).toBeInstanceOf(SupersededError);
		expect(write).toHaveBeenCalledTimes(1);
	});

	it('waits no longer than it was told when the clock jumps back', async () => {
		const { write, pending } = heldUnderU1('96', 5);
		await vi.advanceTimersByTimeAsync(0);
		vi.setSystemTime(Date.now() - 60_000);
		await vi.advanceTimersByTimeAsync(5000);
		expect(await pending).toBe('saved');
		expect(write).toHaveBeenCalledTimes(2);
	});

	it("goes at the end of its wait once the sign-in was marked expired for the same user, and its refusal is the caller's to report", async () => {
		// A read's refused bearer, with no renewal possible, marks the
		// sign-in expired: the same user, so the held write is still that
		// user's. It goes when its wait ends; the tracker's 401 comes back
		// to the caller as the failure it is, never dropped as superseded.
		const unauthorized = Object.assign(new Error('unauthorized'), { status: 401, detail: '' });
		const write = vi
			.fn<() => Promise<string>>()
			.mockRejectedValueOnce(refused(5))
			.mockRejectedValueOnce(unauthorized);
		const pending = saveWhenTrackerAllows('anilist', '97', write, undefined, { user: 'u1' }).catch(
			(e: unknown) => e
		);
		await vi.advanceTimersByTimeAsync(0);
		accountStore.setExpired('anilist', as('u1'));
		await vi.advanceTimersByTimeAsync(5000);
		expect(await pending).toBe(unauthorized);
		expect(write).toHaveBeenCalledTimes(2);
		expect(toastStore.items.filter((t) => t.kind === 'info')).toEqual([]);
	});

	it('keeps waiting when its sign-in is renewed for the same user, and goes at the end', async () => {
		const { write, pending } = heldUnderU1('98', 5);
		await vi.advanceTimersByTimeAsync(0);
		accountStore.setConnected('anilist', { ...as('u1'), access_token: 'renewed' });
		await vi.advanceTimersByTimeAsync(5000);
		expect(await pending).toBe('saved');
		expect(write).toHaveBeenCalledTimes(2);
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

describe('a held write and its notice, on one clock', () => {
	beforeEach(() => {
		vi.useFakeTimers();
	});
	afterEach(() => {
		vi.useRealTimers();
	});
	const going = () => m.account_busy_saving_now({ provider: m.account_provider_anilist() });

	it('goes at the first wake-up past its deadline, however late the timers wake, as its notice says', async () => {
		// A minimised window's timers may wake a minute late each: the wait
		// is a deadline, not sixty one-second steps that each run late.
		const write = vi
			.fn<() => Promise<string>>()
			.mockRejectedValueOnce(refused(60))
			.mockResolvedValue('saved');
		const pending = saveWhenTrackerAllows('anilist', '70', write);
		await vi.advanceTimersByTimeAsync(0);
		runClockAhead(60_000);
		await vi.advanceTimersByTimeAsync(1000);
		expect(write).toHaveBeenCalledTimes(2);
		expect(await pending).toBe('saved');
	});

	it('neither goes nor says it is going when the wall clock jumps past its deadline', async () => {
		let land: (v: string) => void = () => {};
		const write = vi
			.fn<() => Promise<string>>()
			.mockRejectedValueOnce(refused(60))
			.mockImplementationOnce(() => new Promise<string>((r) => (land = r)));
		const pending = saveWhenTrackerAllows('anilist', '71', write);
		await vi.advanceTimersByTimeAsync(0);
		vi.setSystemTime(Date.now() + 120_000);
		await vi.advanceTimersByTimeAsync(1000);
		expect(write).toHaveBeenCalledTimes(1);
		expect(busyNotices().map((t) => t.message)).toEqual([
			m.account_busy_saving({ provider: m.account_provider_anilist(), seconds: 59 })
		]);
		await vi.advanceTimersByTimeAsync(59_000);
		expect(write).toHaveBeenCalledTimes(2);
		expect(busyNotices().map((t) => t.message)).toEqual([going()]);
		land('saved');
		expect(await pending).toBe('saved');
	});
});

describe('what the notice announces', () => {
	afterEach(() => {
		vi.useRealTimers();
	});

	it('announces its first words, not each second, and then that the save is going', async () => {
		vi.useFakeTimers();
		const id = noticeBusy('anilist', 3, 'anilist');
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
