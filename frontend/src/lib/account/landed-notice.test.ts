/**
 * The notice after an account action whose bridge call rejected. The
 * disk is read again to learn whether the action landed after all; a
 * keyring that answers at once decides whether the notice shows, and a
 * keyring that takes its time does not hold the notice back.
 */

import { describe, expect, it, vi } from 'vitest';
import { noticeUnlessLanded, realTimers } from './landed-notice';

function harness(landed: boolean) {
	const timers: { fn: () => void; ms: number; cleared: boolean }[] = [];
	const pushed: string[] = [];
	const dismissed: string[] = [];
	let answer: (applied: boolean) => void = () => {};
	const run = noticeUnlessLanded({
		reread: () => new Promise<boolean>((resolve) => (answer = resolve)),
		landed: () => landed,
		push: () => {
			pushed.push(`toast-${pushed.length}`);
			return `toast-${pushed.length - 1}`;
		},
		dismiss: (id) => dismissed.push(id),
		waitMs: 2000,
		setTimer: (fn, ms) => {
			const t = { fn, ms, cleared: false };
			timers.push(t);
			return t;
		},
		clearTimer: (h) => {
			(h as { cleared: boolean }).cleared = true;
		}
	});
	const fire = () => {
		for (const t of timers) if (!t.cleared) t.fn();
	};
	return { run, answer: (a: boolean) => answer(a), fire, pushed, dismissed, timers };
}

describe('noticeUnlessLanded', () => {
	it('waits no longer than the keyring wait before deciding', () => {
		const h = harness(false);
		expect(h.timers.map((t) => t.ms)).toEqual([2000]);
		h.answer(true);
	});

	it('a disk that answers at once and shows the action landed: no notice', async () => {
		const h = harness(true);
		h.answer(true);
		await h.run;
		expect(h.pushed).toEqual([]);
		expect(h.timers[0].cleared).toBe(true);
	});

	it('a disk that answers at once and shows it did not: one notice', async () => {
		const h = harness(false);
		h.answer(true);
		await h.run;
		expect(h.pushed).toEqual(['toast-0']);
	});

	it('a read that could not be applied: one notice', async () => {
		const h = harness(true);
		h.answer(false);
		await h.run;
		expect(h.pushed).toEqual(['toast-0']);
	});

	it('a keyring that takes its time: the notice shows at the wait, and once', async () => {
		const h = harness(false);
		h.fire();
		expect(h.pushed).toEqual(['toast-0']);
		h.answer(true);
		await h.run;
		expect(h.pushed).toEqual(['toast-0']);
		expect(h.dismissed).toEqual([]);
	});

	it('a keyring that takes its time and then shows the action landed withdraws the notice', async () => {
		const h = harness(true);
		h.fire();
		h.answer(true);
		await h.run;
		expect(h.dismissed).toEqual(['toast-0']);
	});

	it('never rejects', async () => {
		const run = noticeUnlessLanded({
			reread: async () => true,
			landed: () => true,
			push: vi.fn(() => 'id'),
			dismiss: vi.fn(),
			waitMs: 1,
			setTimer: (fn, ms) => setTimeout(fn, ms),
			clearTimer: (h) => clearTimeout(h as ReturnType<typeof setTimeout>)
		});
		await expect(run).resolves.toBeUndefined();
	});

	it('the real timers fire once their time is up, and not once cleared', () => {
		vi.useFakeTimers();
		try {
			const fired = vi.fn();
			const kept = vi.fn();
			realTimers.setTimer(kept, 10);
			realTimers.clearTimer(realTimers.setTimer(fired, 10));
			vi.advanceTimersByTime(10);
			expect(kept).toHaveBeenCalledTimes(1);
			expect(fired).not.toHaveBeenCalled();
		} finally {
			vi.useRealTimers();
		}
	});
});
