// @vitest-environment happy-dom
import fc from 'fast-check';
import { describe, expect, it, vi } from 'vitest';
import {
	HERO_ROTATE_MS,
	nextHeroIndex,
	shouldRunHeroRotation,
	startHeroRotation,
	type IntervalHost
} from './hero-rotation';

describe('nextHeroIndex', () => {
	it('advances within bounds', () => {
		expect(nextHeroIndex(0, 3)).toBe(1);
		expect(nextHeroIndex(1, 3)).toBe(2);
	});

	it('wraps at the end of the rotation', () => {
		expect(nextHeroIndex(2, 3)).toBe(0);
	});

	it('returns 0 for an empty rotation (defensive)', () => {
		// Caller might call us with total=0 if the trending row hasn't
		// loaded yet. Don't blow up; let the next render figure it out.
		expect(nextHeroIndex(0, 0)).toBe(0);
		expect(nextHeroIndex(7, 0)).toBe(0);
	});
});

describe('shouldRunHeroRotation', () => {
	const ok = {
		rotationLength: 3,
		paused: false,
		prefersReducedMotion: false
	};

	it('runs in the happy path', () => {
		expect(shouldRunHeroRotation(ok)).toBe(true);
	});

	it('does not run for a single-item rotation', () => {
		// Nothing to rotate to.
		expect(shouldRunHeroRotation({ ...ok, rotationLength: 1 })).toBe(false);
		expect(shouldRunHeroRotation({ ...ok, rotationLength: 0 })).toBe(false);
	});

	it('does not run while paused (hover / focus held)', () => {
		expect(shouldRunHeroRotation({ ...ok, paused: true })).toBe(false);
	});

	it('does not run when the user prefers reduced motion', () => {
		expect(shouldRunHeroRotation({ ...ok, prefersReducedMotion: true })).toBe(false);
	});
});

describe('startHeroRotation', () => {
	/** An interval host whose ticks run only when the test says so. */
	function fakeIntervals() {
		let nextId = 1;
		const live = new Map<number, { cb: () => void; ms: number }>();
		const host: IntervalHost = {
			every: (cb, ms) => {
				const id = nextId++;
				live.set(id, { cb, ms });
				return id;
			},
			cancel: (id) => live.delete(id)
		};
		const tick = () => [...live.values()].forEach((t) => t.cb());
		return { host, live, tick };
	}

	const running = { rotationLength: 3, paused: false, prefersReducedMotion: false };

	it('advances once per tick, every HERO_ROTATE_MS, until stopped', () => {
		const f = fakeIntervals();
		const advance = vi.fn();
		const stop = startHeroRotation(running, advance, f.host);
		expect([...f.live.values()].map((t) => t.ms)).toEqual([HERO_ROTATE_MS]);
		f.tick();
		f.tick();
		expect(advance).toHaveBeenCalledTimes(2);
		stop();
		expect(f.live.size).toBe(0);
		f.tick();
		expect(advance).toHaveBeenCalledTimes(2);
	});

	it('starts nothing when the rotation should not run, and its stop is harmless', () => {
		const f = fakeIntervals();
		const advance = vi.fn();
		const stop = startHeroRotation({ ...running, prefersReducedMotion: true }, advance, f.host);
		expect(f.live.size).toBe(0);
		stop();
		expect(advance).not.toHaveBeenCalled();
	});

	it('runs an interval exactly when shouldRunHeroRotation says so, for any conditions', () => {
		fc.assert(
			fc.property(
				fc.integer({ min: 0, max: 10 }),
				fc.boolean(),
				fc.boolean(),
				fc.integer({ min: 0, max: 5 }),
				(rotationLength, paused, prefersReducedMotion, ticks) => {
					const conditions = { rotationLength, paused, prefersReducedMotion };
					const f = fakeIntervals();
					const advance = vi.fn();
					const stop = startHeroRotation(conditions, advance, f.host);
					const runs = shouldRunHeroRotation(conditions);
					expect(f.live.size).toBe(runs ? 1 : 0);
					for (let i = 0; i < ticks; i++) f.tick();
					expect(advance).toHaveBeenCalledTimes(runs ? ticks : 0);
					stop();
					expect(f.live.size).toBe(0);
				}
			)
		);
	});

	it('uses the window interval, never with the host as `this`', () => {
		// Chromium throws "Illegal invocation" when a web global runs with
		// another object as `this`; happy-dom does not, so the receiver is
		// checked directly.
		vi.useFakeTimers();
		try {
			const every = vi.spyOn(window, 'setInterval');
			const cancel = vi.spyOn(window, 'clearInterval');
			const advance = vi.fn();
			const stop = startHeroRotation(running, advance);
			vi.advanceTimersByTime(HERO_ROTATE_MS * 2 + 10);
			expect(advance).toHaveBeenCalledTimes(2);
			stop();
			vi.advanceTimersByTime(HERO_ROTATE_MS * 2);
			expect(advance).toHaveBeenCalledTimes(2);
			expect(every.mock.contexts).toEqual([window]);
			expect(cancel.mock.contexts).toEqual([window]);
		} finally {
			vi.restoreAllMocks();
			vi.useRealTimers();
		}
	});
});
