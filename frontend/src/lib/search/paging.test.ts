import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import type { KitsuAnimeRef } from '$lib/api';
import {
	MAX_EMPTY_PAGES,
	afterPage,
	canLoadMore,
	freshHits,
	shouldAutoLoad,
	type PagingPhase
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

describe('shouldAutoLoad', () => {
	it('loads by itself only when idle with the bottom of the grid in view', () => {
		const auto = PHASES.filter((p) => shouldAutoLoad(p, true)).map((p) => p.kind);
		expect(auto).toEqual(['idle']);
		expect(PHASES.some((p) => shouldAutoLoad(p, false))).toBe(false);
	});
});
