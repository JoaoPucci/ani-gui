import { describe, expect, it } from 'vitest';

import { createFreshness, type FreshnessState } from './watch-later-freshness';
import { nextAgeChange } from './watch-later-status';

const MIN = 60_000;
const NOW = 1_800_000_000_000;

/** A clock the test moves by hand, with one list of pending timers. */
function fakeClock(start = NOW) {
	let now = start;
	let nextId = 1;
	const pending = new Map<number, { at: number; fn: () => void }>();
	return {
		pending,
		now: () => now,
		schedule(fn: () => void, ms: number) {
			const id = nextId++;
			pending.set(id, { at: now + ms, fn });
			return id;
		},
		cancel(handle: unknown) {
			pending.delete(handle as number);
		},
		/** Move the clock to `to`, firing every timer due by then, in order. */
		advance(to: number) {
			for (;;) {
				const due = [...pending.entries()]
					.filter(([, t]) => t.at <= to)
					.sort((a, b) => a[1].at - b[1].at)[0];
				if (!due) break;
				pending.delete(due[0]);
				now = due[1].at;
				due[1].fn();
			}
			now = to;
		}
	};
}

function setup(start = NOW) {
	const clock = fakeClock(start);
	const seen: FreshnessState[] = [];
	const freshness = createFreshness(clock, (s) => seen.push(s));
	return { clock, seen, freshness };
}

describe('the age timer', () => {
	it('fires when the age on screen next changes, and re-arms for the change after', () => {
		const { clock, freshness } = setup();
		const at = NOW - 30_000;
		freshness.track({ kind: 'updated', at });
		expect([...clock.pending.values()].map((t) => t.at)).toEqual([NOW + nextAgeChange(at, NOW)]);
		clock.advance(NOW + nextAgeChange(at, NOW));
		const first = freshness.state.now;
		expect(first).toBe(at + MIN);
		expect([...clock.pending.values()].map((t) => t.at)).toEqual([
			first + nextAgeChange(at, first)
		]);
		clock.advance(at + 3 * MIN);
		expect(freshness.state.now).toBe(at + 3 * MIN);
		expect(clock.pending.size).toBe(1);
	});

	it('holds no timer while the line shows no age', () => {
		const { clock, freshness } = setup();
		freshness.track({ kind: 'updated', at: NOW });
		for (const status of [
			{ kind: 'refreshing' } as const,
			{ kind: 'never' } as const,
			{ kind: 'failed' } as const,
			null
		]) {
			freshness.track(status);
			expect(clock.pending.size, String(status?.kind)).toBe(0);
		}
	});

	it('replaces the timer when the status changes, never stacking a second', () => {
		const { clock, freshness } = setup();
		freshness.track({ kind: 'updated', at: NOW - 10 * MIN });
		freshness.track({ kind: 'updated', at: NOW - 2 * 60 * MIN });
		expect(clock.pending.size).toBe(1);
	});
});

describe('a refresh', () => {
	it('holds the line’s label and announces itself when it starts', () => {
		const { freshness } = setup();
		freshness.refreshStarted('Updated 5 minutes ago', 'Refreshing…');
		expect(freshness.state).toMatchObject({
			held: 'Updated 5 minutes ago',
			announce: 'Refreshing…'
		});
	});

	it('when it settles, moves the clock, has the times read again, and announces the line as it then reads', () => {
		const { clock, freshness, seen } = setup();
		freshness.refreshStarted('Updated 5 minutes ago', 'Refreshing…');
		clock.advance(NOW + 4_000);
		let readAt: FreshnessState | null = null;
		freshness.refreshSettled(() => {
			// The label is read once the settle has landed.
			readAt = freshness.state;
			return 'Updated just now';
		});
		expect(readAt).toMatchObject({ now: NOW + 4_000, stamped: 1, held: null });
		expect(freshness.state).toEqual({
			now: NOW + 4_000,
			stamped: 1,
			held: null,
			announce: 'Updated just now'
		});
		expect(seen.at(-1)).toEqual(freshness.state);
	});

	it('announces nothing when the line has nothing to say after it', () => {
		const { freshness } = setup();
		freshness.refreshStarted(null, 'Refreshing…');
		freshness.refreshSettled(() => null);
		expect(freshness.state.announce).toBe('');
	});
});

describe('the last-pulled times changing', () => {
	it('has the times read again, and nothing else', () => {
		const { freshness } = setup();
		freshness.refreshStarted('Updated 5 minutes ago', 'Refreshing…');
		const before = freshness.state;
		freshness.stampsChanged();
		expect(freshness.state).toEqual({ ...before, stamped: before.stamped + 1 });
	});
});

describe('teardown', () => {
	it('cancels the pending timer, and nothing changes afterwards', () => {
		const { clock, freshness, seen } = setup();
		freshness.track({ kind: 'updated', at: NOW });
		freshness.dispose();
		expect(clock.pending.size).toBe(0);
		const count = seen.length;
		freshness.stampsChanged();
		freshness.track({ kind: 'updated', at: NOW });
		freshness.refreshSettled(() => 'x');
		clock.advance(NOW + 10 * MIN);
		expect(seen.length).toBe(count);
		expect(clock.pending.size).toBe(0);
	});
});
