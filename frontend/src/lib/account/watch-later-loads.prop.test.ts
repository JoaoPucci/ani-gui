import { describe, expect, it } from 'vitest';
import fc from 'fast-check';

import { createRailLoads, type LoadTicket } from './watch-later-loads';

/** One step: start a load, finish the `n`th started one (modulo how
 *  many have started), or ask whether the latest has settled. */
type Step = { op: 'start' } | { op: 'finish'; n: number } | { op: 'settled' };

const step: fc.Arbitrary<Step> = fc.oneof(
	fc.constant({ op: 'start' as const }),
	fc.nat({ max: 20 }).map((n) => ({ op: 'finish' as const, n })),
	fc.constant({ op: 'settled' as const })
);

describe('createRailLoads settled, by property', () => {
	it('resolves a waiter exactly once the latest load has finished, however loads interleave', async () => {
		await fc.assert(
			fc.asyncProperty(fc.array(step, { maxLength: 30 }), async (steps) => {
				const loads = createRailLoads();
				const tickets: LoadTicket[] = [];
				const finished = new Set<number>();
				// Each waiter, whether it has resolved, and whether the
				// model says it should have: once, after it was taken, the
				// latest load started was one that had finished.
				const waiters: { done: boolean; due: boolean }[] = [];
				const latestFinished = () =>
					tickets.length === 0 || finished.has(tickets[tickets.length - 1].seq);
				for (const s of steps) {
					if (s.op === 'start') tickets.push(loads.start(false));
					else if (s.op === 'finish' && tickets.length > 0) {
						const t = tickets[s.n % tickets.length];
						finished.add(t.seq);
						loads.finish(t);
					} else if (s.op === 'settled') {
						const w = { done: false, due: false };
						waiters.push(w);
						void loads.settled().then(() => (w.done = true));
					}
					if (latestFinished()) for (const w of waiters) w.due = true;
					await Promise.resolve();
					await Promise.resolve();
					for (const w of waiters) expect(w.done).toBe(w.due);
				}
			})
		);
	});
});
