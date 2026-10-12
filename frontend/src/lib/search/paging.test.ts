import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import type { KitsuAnimeRef } from '$lib/api';
import {
	MAX_EMPTY_PAGES,
	afterPage,
	canLoadMore,
	freshHits,
	shouldAutoLoad,
	UNMEASURED,
	measured,
	spent,
	type PagingPhase,
	type Reach
} from './paging';

const ref = (id: string) => ({ id, canonical_title: `Title ${id}` }) as KitsuAnimeRef;

const PHASES: PagingPhase[] = [
	{ kind: 'loading' },
	{ kind: 'idle' },
	{ kind: 'loading-more' },
	{ kind: 'end' },
	{ kind: 'failed', error: new Error('x') },
	{ kind: 'held' }
];

describe('afterPage', () => {
	it('a page with something to show settles idle while more pages exist', () => {
		expect(afterPage(3, 40, 2)).toEqual({
			phase: { kind: 'idle' },
			emptyStreak: 0,
			advance: false
		});
	});

	it('the last page ends the search, shown or not', () => {
		expect(afterPage(3, null, 0)).toEqual({
			phase: { kind: 'end' },
			emptyStreak: 0,
			advance: false
		});
		expect(afterPage(0, null, 1).phase).toEqual({ kind: 'end' });
		expect(afterPage(0, null, 1).advance).toBe(false);
	});

	it('an empty page with more behind it loads the next one by itself', () => {
		expect(afterPage(0, 20, 0)).toEqual({
			phase: { kind: 'loading-more' },
			emptyStreak: 1,
			advance: true
		});
	});

	it('stops advancing on its own after MAX_EMPTY_PAGES empty pages in a row', () => {
		let streak = 0;
		let advances = 0;
		for (let i = 0; i < 50; i++) {
			const r = afterPage(0, (i + 1) * 20, streak);
			streak = r.emptyStreak;
			if (!r.advance) {
				expect(r.phase).toEqual({ kind: 'held' });
				break;
			}
			advances += 1;
		}
		expect(advances).toBe(MAX_EMPTY_PAGES - 1);
		expect(streak).toBe(MAX_EMPTY_PAGES);
	});

	it('never advances without a next page, and never past the bound (property)', () => {
		fc.assert(
			fc.property(
				fc.nat(40),
				fc.option(fc.nat(1000), { nil: null }),
				fc.nat(MAX_EMPTY_PAGES - 1),
				(visible, next, streak) => {
					const r = afterPage(visible, next, streak);
					if (r.advance) {
						expect(next).not.toBeNull();
						expect(visible).toBe(0);
						expect(r.emptyStreak).toBeLessThan(MAX_EMPTY_PAGES);
					}
					if (visible > 0) expect(r.emptyStreak).toBe(0);
				}
			)
		);
	});
});

describe('freshHits', () => {
	it('drops hits an earlier page already carried, keeping the first appearance', () => {
		const seen = new Set<string>();
		expect(freshHits(seen, [ref('1'), ref('2')]).map((h) => h.id)).toEqual(['1', '2']);
		expect(freshHits(seen, [ref('2'), ref('3'), ref('3')]).map((h) => h.id)).toEqual(['3']);
		expect([...seen].sort()).toEqual(['1', '2', '3']);
	});
});

describe('canLoadMore', () => {
	it('only an idle, failed or held search can ask for more', () => {
		const can = PHASES.filter(canLoadMore).map((p) => p.kind);
		expect(can).toEqual(['idle', 'failed', 'held']);
	});
});

const IN_REACH = measured(true);

describe('shouldAutoLoad', () => {
	it('loads by itself only when idle on a fresh in-reach measurement', () => {
		const auto = PHASES.filter((p) => shouldAutoLoad(p, IN_REACH, false)).map((p) => p.kind);
		expect(auto).toEqual(['idle']);
		expect(PHASES.some((p) => shouldAutoLoad(p, measured(false), false))).toBe(false);
		expect(PHASES.some((p) => shouldAutoLoad(p, UNMEASURED, false))).toBe(false);
	});

	it('never loads by itself while a type filter narrows the grid', () => {
		// A narrowed grid stays short, so the bottom stays in reach and
		// every settled page would fetch the next: a crawl through the
		// whole search, unbounded by the empty-page hold because the
		// pages are not empty, only filtered out of view.
		expect(PHASES.some((p) => shouldAutoLoad(p, IN_REACH, true))).toBe(false);
	});

	it('a spent measurement loads nothing until a new one arrives', () => {
		// One intersection, one page: the measurement that started a
		// load, or that predates a settled page, says where the marker
		// was before the grid grew.
		const idle: PagingPhase = { kind: 'idle' };
		expect(shouldAutoLoad(idle, spent(IN_REACH), false)).toBe(false);
		expect(shouldAutoLoad(idle, spent(spent(IN_REACH)), false)).toBe(false);
		expect(shouldAutoLoad(idle, measured(true), false)).toBe(true);
	});

	it('only a measurement taken after the last spend can load (property)', () => {
		// Any sequence of measurements and spends: a load is allowed
		// exactly when the last step was an in-reach measurement.
		fc.assert(
			fc.property(fc.array(fc.oneof(fc.constant<'spend'>('spend'), fc.boolean())), (steps) => {
				let reach: Reach = UNMEASURED;
				for (const step of steps) reach = step === 'spend' ? spent(reach) : measured(step);
				const last = steps.at(-1);
				expect(shouldAutoLoad({ kind: 'idle' }, reach, false)).toBe(last === true);
			})
		);
	});
});
