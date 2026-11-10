import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import {
	discoveryView,
	LOADING,
	PENDING,
	settleRow,
	type RowLoad,
	type RowSlot
} from './discovery';

const ready = (...items: string[]): RowLoad<string> => ({ kind: 'ready', items });
const FAILED: RowLoad<string> = { kind: 'failed' };
const SKELETON: RowSlot<string> = { kind: 'skeleton' };
const ABSENT: RowSlot<string> = { kind: 'absent' };
const shown = (...items: string[]): RowSlot<string> => ({ kind: 'shown', items });
const HELD: RowSlot<string> = { kind: 'held' };

describe('discoveryView', () => {
	it('holds a skeleton in every row while both are loading', () => {
		expect(discoveryView([LOADING, LOADING])).toEqual({
			slots: [SKELETON, SKELETON],
			nothing: false
		});
	});

	it('keeps a row that has landed behind a skeleton while a row above it is still loading', () => {
		// Top rated is usually cached longer than Trending, so it lands
		// first; drawn on its own it would be pushed down when Trending
		// arrives above it.
		expect(discoveryView([LOADING, ready('r1')]).slots).toEqual([SKELETON, SKELETON]);
	});

	it('shows both rows together once the row above lands', () => {
		expect(discoveryView([ready('t1'), ready('r1')]).slots).toEqual([shown('t1'), shown('r1')]);
	});

	it('shows the top row as soon as it lands, with the row below still a skeleton', () => {
		// Nothing above it can move, and the skeleton below fills in
		// place.
		expect(discoveryView([ready('t1'), LOADING]).slots).toEqual([shown('t1'), SKELETON]);
	});

	it('drops a failed row and lets the row below show', () => {
		expect(discoveryView([FAILED, ready('r1')])).toEqual({
			slots: [ABSENT, shown('r1')],
			nothing: false
		});
	});

	it('drops a row the availability filter emptied', () => {
		expect(discoveryView([ready(), ready('r1')]).slots).toEqual([ABSENT, shown('r1')]);
	});

	it('drops a failed row below a loading one rather than holding a skeleton for it', () => {
		expect(discoveryView([LOADING, FAILED]).slots).toEqual([SKELETON, ABSENT]);
	});

	it('says there is nothing to browse when every row failed', () => {
		expect(discoveryView([FAILED, FAILED])).toEqual({ slots: [ABSENT, ABSENT], nothing: true });
	});

	it('says there is nothing to browse when every row came back empty', () => {
		expect(discoveryView([ready(), ready()]).nothing).toBe(true);
	});

	it('says so when one row came back empty and the other failed', () => {
		expect(discoveryView([ready(), FAILED]).nothing).toBe(true);
	});

	it('does not say so while a row is still loading', () => {
		expect(discoveryView([FAILED, LOADING]).nothing).toBe(false);
	});
});

describe('discoveryView, with a row not yet known to exist', () => {
	// A row whose presence is not known yet (the home page's Continue
	// watching before the history read answers) has no placeholder of
	// its own. Anything drawn below it would be pushed down when it
	// turns out to be there, so nothing below it is drawn at all.
	it('draws nothing in or below a pending row', () => {
		expect(discoveryView([PENDING, LOADING, ready('r1')])).toEqual({
			slots: [HELD, HELD, HELD],
			nothing: false
		});
	});

	it('leaves the rows above a pending row as they are', () => {
		expect(discoveryView([ready('t1'), PENDING, ready('r1')]).slots).toEqual([
			shown('t1'),
			HELD,
			HELD
		]);
	});

	it('draws the rows below once the pending row turns out to be absent', () => {
		expect(discoveryView([ready(), LOADING, ready('r1')]).slots).toEqual([
			ABSENT,
			SKELETON,
			SKELETON
		]);
	});

	it('does not say there is nothing to browse while a row is pending', () => {
		expect(discoveryView([FAILED, PENDING]).nothing).toBe(false);
	});
});

describe('discoveryView, as rows settle in any order', () => {
	const settled = fc.oneof(
		fc.constant<RowLoad<string>>(FAILED),
		fc.array(fc.string({ maxLength: 3 }), { maxLength: 3 }).map((items) => ready(...items))
	);
	// The rows' final answers, whether each starts out pending (not yet
	// known to exist) rather than loading, and the order of the steps:
	// a pending row's index appears twice, once to learn it exists (it
	// is then loading, or settles straight away when it failed) and
	// once to settle.
	const run = fc
		.array(fc.tuple(settled, fc.boolean()), { minLength: 1, maxLength: 5 })
		.chain((rows) => {
			const steps = rows.flatMap(([, pending], i) => (pending ? [i, i] : [i]));
			return fc
				.shuffledSubarray(steps, { minLength: steps.length, maxLength: steps.length })
				.map((order) => ({
					finals: rows.map(([f]) => f),
					pending: rows.map(([, p]) => p),
					order
				}));
		});

	/** Every view the page draws on the way, from nothing known to all
	 *  settled. */
	function views(finals: RowLoad<string>[], order: number[], pending: boolean[] = []) {
		const loads: RowLoad<string>[] = finals.map((_, i) => (pending[i] ? PENDING : LOADING));
		const seen = [discoveryView(loads)];
		for (const i of order) {
			loads[i] = loads[i].kind === 'pending' && finals[i].kind !== 'failed' ? LOADING : finals[i];
			seen.push(discoveryView([...loads]));
		}
		return seen;
	}

	it('never moves a row once it is shown', () => {
		fc.assert(
			fc.property(run, ({ finals, order, pending }) => {
				const seen = views(finals, order, pending);
				seen.forEach((view, k) => {
					view.slots.forEach((slot, i) => {
						if (slot.kind !== 'shown') return;
						// Everything drawn from the top down to this row stays
						// exactly as it is in every later view.
						for (const later of seen.slice(k + 1)) {
							expect(later.slots.slice(0, i + 1)).toEqual(view.slots.slice(0, i + 1));
						}
					});
				});
			})
		);
	});

	it('never draws a row above one already drawn, so nothing drawn is pushed down', () => {
		const drawn = (slot: RowSlot<string>) => slot.kind === 'skeleton' || slot.kind === 'shown';
		const drawnAbove = (slots: RowSlot<string>[], i: number) =>
			slots.slice(0, i).filter(drawn).length;
		fc.assert(
			fc.property(run, ({ finals, order, pending }) => {
				const seen = views(finals, order, pending);
				seen.forEach((view, k) => {
					view.slots.forEach((slot, i) => {
						if (!drawn(slot)) return;
						for (const later of seen.slice(k + 1)) {
							expect(drawnAbove(later.slots, i)).toBeLessThanOrEqual(drawnAbove(view.slots, i));
						}
					});
				});
			})
		);
	});

	it('ends with every row that has something to show, shown in its place', () => {
		fc.assert(
			fc.property(run, ({ finals, order, pending }) => {
				const last = views(finals, order, pending).at(-1)!;
				const expected = finals.map((f) =>
					f.kind === 'ready' && f.items.length > 0 ? shown(...f.items) : ABSENT
				);
				expect(last.slots).toEqual(expected);
				expect(last.nothing).toBe(expected.every((s) => s.kind === 'absent'));
			})
		);
	});
});

describe('settleRow', () => {
	it('settles an answer as a ready row', async () => {
		await expect(settleRow(Promise.resolve(['a']))).resolves.toEqual(ready('a'));
	});

	it('settles a failure as a failed row instead of rejecting', async () => {
		await expect(settleRow(Promise.reject(new Error('offline')))).resolves.toEqual(FAILED);
	});
});
