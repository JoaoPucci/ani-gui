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

describe('the countdown, by property', () => {
	it('never shows more than the time truly remaining, nor less than zero', () => {
		fc.assert(
			fc.property(
				fc.integer({ min: -5000, max: 600_000 }),
				fc.integer({ min: 0, max: 600_000 }),
				(deadline, now) => {
					const s = secondsLeft(deadline, now);
					expect(s).toBeGreaterThanOrEqual(0);
					expect(s * 1000).toBeLessThanOrEqual(Math.max(0, deadline - now));
					// …and is behind it by less than a second.
					expect(Math.max(0, deadline - now) - s * 1000).toBeLessThan(1000);
				}
			)
		);
	});

	it('holds over any run of ticks, however late each comes', () => {
		fc.assert(
			fc.property(
				fc.integer({ min: 1, max: 120 }),
				fc.array(fc.integer({ min: 0, max: 5000 }), { maxLength: 40 }),
				(waitSeconds, lateness) => {
					let clock = 0;
					const deadline = waitSeconds * 1000;
					const shown: { at: number; seconds: number }[] = [];
					let tick: (() => void) | null = null;
					startCountdown(deadline, {
						now: () => clock,
						after: (_ms, t) => {
							tick = t;
							return () => {
								tick = null;
							};
						},
						show: (seconds) => shown.push({ at: clock, seconds })
					});
					for (const late of lateness) {
						if (!tick) break;
						// A one-shot timer: it fires once, and the countdown re-arms it.
						const fire = tick as () => void;
						tick = null;
						clock += 1000 + late;
						fire();
					}
					for (const { at, seconds } of shown) {
						expect(seconds).toBeGreaterThanOrEqual(0);
						expect(seconds * 1000).toBeLessThanOrEqual(Math.max(0, deadline - at));
					}
					// Once zero is shown, nothing more is.
					const zero = shown.findIndex((e) => e.seconds === 0);
					if (zero !== -1) expect(shown).toHaveLength(zero + 1);
				}
			)
		);
	});
});
