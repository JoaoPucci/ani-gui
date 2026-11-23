import { describe, expect, it } from 'vitest';
import { EPISODE_WAIT_CAP_MS } from './card-look';
import { createCapTimers } from './cap-timers';

describe('createCapTimers', () => {
	function fakeClock() {
		const pending = new Map<number, () => void>();
		let next = 0;
		return {
			pending,
			schedule: (fn: () => void) => {
				pending.set(++next, fn);
				return next;
			},
			cancel: (h: unknown) => pending.delete(h as number),
			fire: () => {
				for (const [h, fn] of [...pending]) {
					pending.delete(h);
					fn();
				}
			}
		};
	}

	it('runs one wait per card, counted from the first arm', () => {
		const clock = fakeClock();
		const elapsed: string[] = [];
		const timers = createCapTimers({ capMs: 400, ...clock, onElapsed: (id) => elapsed.push(id) });
		timers.arm('a');
		timers.arm('a');
		timers.arm('b');
		expect(clock.pending.size).toBe(2);
		clock.fire();
		expect(elapsed).toEqual(['a', 'b']);
		timers.arm('a');
		expect(clock.pending.size).toBe(0);
	});

	it('says whether a wait is running for the key, so a caller can arm as it reads', () => {
		const clock = fakeClock();
		const timers = createCapTimers({ capMs: 400, ...clock, onElapsed: () => {} });
		expect(timers.arm('a|4')).toBe(true);
		expect(timers.arm('a|4')).toBe(true);
		// The card offers another episode: that one gets its own full wait.
		expect(timers.arm('a|5')).toBe(true);
		expect(clock.pending.size).toBe(2);
		timers.dispose();
		expect(timers.arm('a|6')).toBe(false);
	});

	it('cancels every wait when the page goes, and arms none after', () => {
		const clock = fakeClock();
		const elapsed: string[] = [];
		const timers = createCapTimers({ capMs: 400, ...clock, onElapsed: (id) => elapsed.push(id) });
		timers.arm('a');
		timers.dispose();
		timers.arm('b');
		expect(clock.pending.size).toBe(0);
		expect(elapsed).toEqual([]);
	});

	it('waits the cap it is given', () => {
		const delays: number[] = [];
		const timers = createCapTimers({
			capMs: EPISODE_WAIT_CAP_MS,
			schedule: (_fn, ms) => delays.push(ms),
			cancel: () => {},
			onElapsed: () => {}
		});
		timers.arm('a');
		expect(delays).toEqual([EPISODE_WAIT_CAP_MS]);
	});
});
