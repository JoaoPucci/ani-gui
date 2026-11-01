import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import type { KitsuAnimeRef } from '$lib/api';
import type { PagingPhase } from './paging';
import {
	anchorShown,
	checkpointAt,
	checkpointOf,
	restoredState,
	resumePlan,
	resumeTarget
} from './resume';
import { captureSearch, FRESH_FOR_MS } from './snapshot';

const ref = (id: string) => ({ id, canonical_title: id }) as KitsuAnimeRef;

const phaseArb = fc.constantFrom<PagingPhase['kind']>('idle', 'loading-more', 'end', 'held');

describe('checkpointAt', () => {
	it('keeps an ended or held search as it is, and any other as idle', () => {
		fc.assert(
			fc.property(phaseArb, fc.nat(3), (kind, streak) => {
				const cp = checkpointAt(
					{ mode: 'dub', seen: new Set(['1']), next: 20, loaded: 2 },
					{ phase: { kind } as PagingPhase, emptyStreak: streak, advance: false }
				);
				expect(cp.phase).toBe(kind === 'end' || kind === 'held' ? kind : 'idle');
				expect(cp).toMatchObject({ mode: 'dub', seen: ['1'], next: 20, pages: 2 });
				expect(cp.emptyStreak).toBe(streak);
			})
		);
	});
});

describe('checkpointOf', () => {
	it('is nothing without a settled search', () => {
		expect(checkpointOf(null)).toBeNull();
		expect(checkpointOf({ settled: null, pages: [[ref('1')]] })).toBeNull();
	});

	it('carries the results every page shows, in order', () => {
		const settled = {
			mode: 'sub' as const,
			seen: [],
			next: null,
			emptyStreak: 0,
			pages: 2,
			phase: 'end' as const
		};
		const cp = checkpointOf({ settled, pages: [[ref('1')], [ref('2'), ref('3')]] });
		expect(cp?.hits.map((h) => h.id)).toEqual(['1', '2', '3']);
	});
});

describe('restoredState', () => {
	it('brings back a checkpoint as the search it was', () => {
		const kept = {
			mode: 'sub' as const,
			hits: [ref('1')],
			seen: ['1', '2'],
			next: 40,
			emptyStreak: 1,
			pages: 2,
			phase: 'idle' as const
		};
		const s = restoredState(kept);
		expect(s.pages).toEqual([[ref('1')]]);
		expect([...s.seen]).toEqual(['1', '2']);
		expect(s).toMatchObject({ next: 40, emptyStreak: 1, loaded: 2 });
		expect(checkpointOf(s)).toEqual(kept);
	});
});

describe('resumeTarget and anchorShown', () => {
	it('aims for the anchored card within at least one page', () => {
		const base = captureSearch({
			q: 'a',
			order: 'relevance',
			subtypes: [],
			checkpoint: null,
			cards: [{ id: '9', top: 0, bottom: 10 }],
			now: 0
		});
		expect(resumeTarget(base)).toEqual({ id: '9', pages: 1 });
		expect(resumeTarget({ ...base, anchor: null, pages: 3 })).toEqual({ id: null, pages: 3 });
	});

	it('finds the anchored card among the results, and nothing without one', () => {
		expect(anchorShown({ id: '2', top: 0 }, [ref('1'), ref('2')])).toBe(true);
		expect(anchorShown({ id: '3', top: 0 }, [ref('1'), ref('2')])).toBe(false);
		expect(anchorShown(null, [ref('1')])).toBe(false);
	});
});

describe('resumePlan', () => {
	const saved = captureSearch({
		q: 'a',
		order: 'year',
		subtypes: [],
		checkpoint: {
			mode: 'sub',
			hits: [ref('1')],
			seen: ['1'],
			next: 20,
			emptyStreak: 0,
			pages: 1,
			phase: 'idle'
		},
		cards: [{ id: '1', top: 0, bottom: 10 }],
		now: 0
	});

	it('keeps the results when they may be reused, and aims for the card otherwise', () => {
		expect(resumePlan(saved, { now: 1, reload: false, mode: 'sub' })).toEqual({
			kept: saved.results
		});
		expect(resumePlan(saved, { now: FRESH_FOR_MS, reload: false, mode: 'sub' })).toEqual({
			target: { id: '1', pages: 1 }
		});
		expect(resumePlan({ ...saved, results: null }, { now: 1, reload: false, mode: 'sub' })).toEqual(
			{ target: { id: '1', pages: 1 } }
		);
	});
});
