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

	describe("while a strip warm is out for the card's show", () => {
		// A clock that runs timers in due order as time is advanced.
		function timeline() {
			let now = 0;
			let next = 0;
			const due = new Map<number, { at: number; fn: () => void }>();
			return {
				schedule: (fn: () => void, ms: number) => {
					due.set(++next, { at: now + ms, fn });
					return next;
				},
				cancel: (h: unknown) => due.delete(h as number),
				pending: () => due.size,
				async advance(to: number) {
					for (;;) {
						const first = [...due]
							.filter(([, t]) => t.at <= to)
							.sort((a, b) => a[1].at - b[1].at)[0];
						if (!first) break;
						due.delete(first[0]);
						now = first[1].at;
						first[1].fn();
						await Promise.resolve();
						await Promise.resolve();
					}
					now = to;
					await new Promise((r) => setTimeout(r, 0));
				}
			};
		}
		function warm() {
			let end: (o: 'answered' | 'failed') => void = () => {};
			const settled = new Promise<'answered' | 'failed'>((r) => (end = r));
			return { settled, end };
		}
		function setup(pendingFor: (show: string) => Promise<'answered' | 'failed'> | null) {
			const clock = timeline();
			const elapsed: string[] = [];
			const timers = createCapTimers({
				capMs: 1000,
				ceilingMs: 3000,
				pendingWarm: pendingFor,
				schedule: clock.schedule,
				cancel: clock.cancel,
				onElapsed: (k) => elapsed.push(k)
			});
			return { clock, elapsed, timers };
		}

		it('runs on past the cap while the warm is out', async () => {
			const w = warm();
			const { clock, elapsed, timers } = setup((s) => (s === '12' ? w.settled : null));
			timers.arm('a|4', '12');
			await clock.advance(2500);
			expect(elapsed).toEqual([]);
		});

		it('ends at the ceiling when the warm is still out', async () => {
			const w = warm();
			const { clock, elapsed, timers } = setup(() => w.settled);
			timers.arm('a|4', '12');
			await clock.advance(2999);
			expect(elapsed).toEqual([]);
			await clock.advance(3000);
			expect(elapsed).toEqual(['a|4']);
		});

		it('keeps waiting, to the ceiling, for the details once the warm answers', async () => {
			// The card's own read shared the warm's lookup or finds its
			// answer kept: its details are on their way.
			const w = warm();
			const { clock, elapsed, timers } = setup(() => w.settled);
			timers.arm('a|4', '12');
			await clock.advance(1500);
			w.end('answered');
			await clock.advance(2900);
			expect(elapsed).toEqual([]);
			await clock.advance(3000);
			expect(elapsed).toEqual(['a|4']);
		});

		it('ends as soon as the warm fails, once past the cap', async () => {
			const w = warm();
			const { clock, elapsed, timers } = setup(() => w.settled);
			timers.arm('a|4', '12');
			await clock.advance(1500);
			w.end('failed');
			await clock.advance(1501);
			expect(elapsed).toEqual(['a|4']);
			await clock.advance(4000);
			expect(elapsed).toEqual(['a|4']);
		});

		it('runs the plain cap for a show no warm is out for', async () => {
			const { clock, elapsed, timers } = setup(() => null);
			timers.arm('a|4', '12');
			timers.arm('b|1');
			await clock.advance(1000);
			expect(elapsed).toEqual(['a|4', 'b|1']);
		});

		it('runs the plain cap when the warm ended before the cap', async () => {
			// The watch answers none for a warm that has ended.
			let out: Promise<'answered' | 'failed'> | null = warm().settled;
			const { clock, elapsed, timers } = setup(() => out);
			timers.arm('a|4', '12');
			await clock.advance(600);
			out = null;
			await clock.advance(1000);
			expect(elapsed).toEqual(['a|4']);
		});

		it('ends nothing once Home has left, whatever the warm does after', async () => {
			const w = warm();
			const { clock, elapsed, timers } = setup(() => w.settled);
			timers.arm('a|4', '12');
			await clock.advance(1500);
			timers.dispose();
			w.end('failed');
			await clock.advance(5000);
			expect(elapsed).toEqual([]);
			expect(clock.pending()).toBe(0);
		});
	});
});
