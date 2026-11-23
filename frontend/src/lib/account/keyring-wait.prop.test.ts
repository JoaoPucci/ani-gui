// KeyringWait, over any schedule of reads: the notice is up exactly
// while some read has run past the wait without settling, and it only
// ever flips — never says the same thing twice in a row.
import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { KeyringWait } from './keyring-wait';

/** Timers on a clock the property drives. */
function manualTimers() {
	let now = 0;
	let next = 1;
	const timers = new Map<number, { at: number; fn: () => void }>();
	return {
		setTimer: (fn: () => void, ms: number) => {
			const id = next++;
			timers.set(id, { at: now + ms, fn });
			return id;
		},
		clearTimer: (h: unknown) => void timers.delete(h as number),
		advanceTo(t: number) {
			for (;;) {
				const due = [...timers.entries()]
					.filter(([, v]) => v.at <= t)
					.sort((a, b) => a[1].at - b[1].at || a[0] - b[0]);
				if (due.length === 0) break;
				const [id, { at, fn }] = due[0];
				timers.delete(id);
				now = at;
				fn();
			}
			now = t;
		}
	};
}

const WAIT = 100;
const read = fc.record({
	start: fc.integer({ min: 0, max: 400 }),
	// null: still out at the end
	length: fc.option(fc.integer({ min: 0, max: 400 }), { nil: null })
});

describe('KeyringWait over any schedule', () => {
	it('is waiting exactly while an overdue read is out, and alternates', async () => {
		await fc.assert(
			fc.asyncProperty(fc.array(read, { maxLength: 6 }), async (reads) => {
				const timers = manualTimers();
				const changes: boolean[] = [];
				const wait = new KeyringWait(timers, (w) => changes.push(w), WAIT);
				const settle: (() => void)[] = reads.map(() => () => {});
				const events = new Set<number>([0]);
				for (const r of reads) {
					events.add(r.start).add(r.start + WAIT);
					if (r.length !== null) events.add(r.start + r.length);
				}
				const times = [...events].sort((a, b) => a - b);
				for (const t of times) {
					timers.advanceTo(t);
					reads.forEach((r, i) => {
						if (r.start === t) {
							wait.track(new Promise<void>((res) => (settle[i] = res)));
						}
					});
					reads.forEach((r, i) => {
						if (r.length !== null && r.start + r.length === t) settle[i]();
					});
					// let settled promises run their handlers
					for (let k = 0; k < 3; k++) await Promise.resolve();
					timers.advanceTo(t);
					const overdueOut = reads.some(
						(r) => r.start + WAIT <= t && (r.length === null || r.start + r.length > t)
					);
					const state = changes.length > 0 && changes[changes.length - 1];
					expect(state).toBe(overdueOut);
				}
				changes.forEach((c, i) => expect(c).toBe(i % 2 === 0));
			})
		);
	});
});
