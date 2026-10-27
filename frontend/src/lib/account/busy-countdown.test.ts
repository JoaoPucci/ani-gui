import { afterEach, describe, expect, it, vi } from 'vitest';
import fc from 'fast-check';

import { secondsLeft, startCountdown } from './busy-countdown';

const realTimer = (ms: number, tick: () => void) => {
	const handle = setTimeout(() => tick(), ms);
	return () => clearTimeout(handle);
};

afterEach(() => {
	vi.useRealTimers();
});

describe('startCountdown', () => {
	it('shows each second down to zero, then stops by itself', () => {
		vi.useFakeTimers();
		const shown: number[] = [];
		startCountdown(Date.now() + 3000, {
			now: () => Date.now(),
			after: realTimer,
			show: (s) => shown.push(s)
		});
		vi.advanceTimersByTime(10_000);
		expect(shown).toEqual([3, 2, 1, 0]);
		expect(vi.getTimerCount()).toBe(0);
	});

	it('changes the number when the second it names is over, and shows zero only at the deadline', () => {
		vi.useFakeTimers();
		const start = Date.now();
		const shown: [number, number][] = [];
		startCountdown(start + 1500, {
			now: () => Date.now(),
			after: realTimer,
			show: (s) => shown.push([Date.now() - start, s])
		});
		vi.advanceTimersByTime(10_000);
		expect(shown).toEqual([
			[0, 2],
			[500, 1],
			[1500, 0]
		]);
	});

	it('stops when told', () => {
		vi.useFakeTimers();
		const shown: number[] = [];
		const stop = startCountdown(Date.now() + 5000, {
			now: () => Date.now(),
			after: realTimer,
			show: (s) => shown.push(s)
		});
		vi.advanceTimersByTime(1000);
		stop();
		vi.advanceTimersByTime(10_000);
		expect(shown).toEqual([5, 4]);
		expect(vi.getTimerCount()).toBe(0);
	});

	it('shows the true remainder after ticks that came late', () => {
		vi.useFakeTimers();
		const shown: number[] = [];
		startCountdown(Date.now() + 10_000, {
			now: () => Date.now(),
			after: realTimer,
			show: (s) => shown.push(s)
		});
		vi.setSystemTime(Date.now() + 6000);
		vi.advanceTimersByTime(1000);
		expect(shown).toEqual([10, 3]);
	});

	it('shows zero at once for a deadline already past, and starts no timer', () => {
		vi.useFakeTimers();
		const shown: number[] = [];
		startCountdown(Date.now() - 1, {
			now: () => Date.now(),
			after: realTimer,
			show: (s) => shown.push(s)
		});
		expect(shown).toEqual([0]);
		expect(vi.getTimerCount()).toBe(0);
	});
});

/** The seconds a notice may say with `remaining` ms left: the remainder
 *  rounded up to the whole second it is in, and zero only once nothing
 *  is left — the moment the save goes. */
function honest(seconds: number, remaining: number): boolean {
	if (remaining <= 0) return seconds === 0;
	return seconds > 0 && (seconds - 1) * 1000 < remaining && remaining <= seconds * 1000;
}

describe('the countdown, by property', () => {
	it('names the second the remainder is in, and says zero only at the deadline', () => {
		fc.assert(
			fc.property(
				fc.integer({ min: -5000, max: 600_000 }),
				fc.integer({ min: 0, max: 600_000 }),
				(deadline, now) => {
					const s = secondsLeft(deadline, now);
					expect(s).toBeGreaterThanOrEqual(0);
					expect(honest(s, deadline - now)).toBe(true);
				}
			)
		);
	});

	it('holds over any run of ticks, however late each comes', () => {
		fc.assert(
			fc.property(
				fc.integer({ min: 1, max: 120_000 }),
				fc.array(fc.integer({ min: 0, max: 5000 }), { maxLength: 40 }),
				(waitMs, lateness) => {
					let clock = 0;
					const shown: { at: number; seconds: number }[] = [];
					let pending: { at: number; tick: () => void } | null = null;
					startCountdown(waitMs, {
						now: () => clock,
						after: (ms, tick) => {
							pending = { at: clock + ms, tick };
							return () => {
								pending = null;
							};
						},
						show: (seconds) => shown.push({ at: clock, seconds })
					});
					for (const late of lateness) {
						const next = pending as { at: number; tick: () => void } | null;
						if (!next) break;
						pending = null;
						clock = next.at + late;
						next.tick();
					}
					for (const { at, seconds } of shown) expect(honest(seconds, waitMs - at)).toBe(true);
					// Once zero is shown, nothing more is.
					const zero = shown.findIndex((e) => e.seconds === 0);
					if (zero !== -1) expect(shown).toHaveLength(zero + 1);
				}
			)
		);
	});

	it('wakes no later than the second it shows is over, when its timer is on time', () => {
		fc.assert(
			fc.property(fc.integer({ min: 1, max: 120_000 }), (waitMs) => {
				let clock = 0;
				const shown: { at: number; seconds: number }[] = [];
				let pending: { at: number; tick: () => void } | null = null;
				startCountdown(waitMs, {
					now: () => clock,
					after: (ms, tick) => {
						pending = { at: clock + ms, tick };
						return () => {
							pending = null;
						};
					},
					show: (seconds) => shown.push({ at: clock, seconds })
				});
				for (let guard = 0; guard < 200; guard += 1) {
					const next = pending as { at: number; tick: () => void } | null;
					if (!next) break;
					pending = null;
					clock = next.at;
					next.tick();
				}
				// Every second from the first number down is shown, and zero
				// exactly at the deadline.
				const first = Math.ceil(waitMs / 1000);
				expect(shown.map((e) => e.seconds)).toEqual(
					Array.from({ length: first + 1 }, (_, i) => first - i)
				);
				expect(shown.at(-1)?.at).toBe(waitMs);
			})
		);
	});
});
