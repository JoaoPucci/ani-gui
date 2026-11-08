import { describe, expect, it, vi } from 'vitest';
import fc from 'fast-check';
import { parseHistoryEpisode } from '$lib/play/episode-number';
import { FIXTURE_RUNS, type FixtureRun } from './fixtures/runs';
import {
	handoffAsk,
	handoffFrom,
	playedIds,
	railHandoffs,
	type Handoff,
	type HandoffMatch
} from './handoff';
import { runOf } from './strip-model';
import { franchiseArb } from './testing/arbitraries';
import type { FranchiseData } from './types';
import { isFinale, upNextFor } from './up-next';

const TODAY = '2026-11-02';
const run = (key: string): FixtureRun => FIXTURE_RUNS.find((r) => r.key === key)!;

/** A fixture run as the backend would serve it for `anchor`. */
function payload(key: string, anchor: string): FranchiseData {
	const r = run(key);
	return {
		anchor,
		entries: r.entries,
		links: r.links,
		versions: r.versions,
		films: r.films,
		sides: r.sides,
		recaps: r.recaps,
		parents: r.parents[anchor] ? { [anchor]: r.parents[anchor] } : {},
		complete: true
	};
}

const finished = (episodes: number | null, id = '43078'): HandoffMatch => ({
	id,
	status: 'finished',
	episode_count: episodes
});
const never = () => false;

describe('handoffAsk', () => {
	it("asks for the series of a finale played through, by the row's match", () => {
		expect(handoffAsk(finished(13), '13', null, never)).toBe('43078');
		// A row past Kitsu's count is at the end too.
		expect(handoffAsk(finished(13), '14', null, never)).toBe('43078');
	});

	it('asks nothing for a row short of the finale', () => {
		expect(handoffAsk(finished(13), '12', null, never)).toBeNull();
		expect(handoffAsk(finished(13), '12.5', null, never)).toBeNull();
		expect(handoffAsk(finished(13), '', null, never)).toBeNull();
		expect(handoffAsk(finished(13), 'abc', null, never)).toBeNull();
	});

	it('asks nothing for an entry still airing or of unknown length', () => {
		expect(handoffAsk({ ...finished(13), status: 'current' }, '13', null, never)).toBeNull();
		expect(handoffAsk({ ...finished(13), status: null }, '13', null, never)).toBeNull();
		expect(handoffAsk(finished(null), '13', null, never)).toBeNull();
	});

	it('asks nothing for a row not matched yet, or that could not be', () => {
		expect(handoffAsk(undefined, '13', null, never)).toBeNull();
		expect(handoffAsk(null, '13', null, never)).toBeNull();
	});

	it('asks nothing when the finale was left part-way, asking about the finale itself', () => {
		const leftPartWay = vi.fn(() => true);
		expect(handoffAsk(finished(13), '13', null, leftPartWay)).toBeNull();
		expect(leftPartWay).toHaveBeenCalledWith('43078', 13);
	});

	it('asks only where the entry is at its finale and was not left part-way (property)', () => {
		fc.assert(
			fc.property(
				fc.record({
					status: fc.constantFrom('finished', 'current', 'tba', null),
					episodes: fc.option(fc.integer({ min: 1, max: 30 }), { nil: null }),
					ep: fc.oneof(
						fc.integer({ min: 0, max: 32 }).map(String),
						fc.constantFrom('', '3.5', '12.5', 'x', '-1', '1-12')
					),
					partWay: fc.boolean()
				}),
				({ status, episodes, ep, partWay }) => {
					const match = { id: '7', status, episode_count: episodes };
					const got = handoffAsk(match, ep, null, () => partWay);
					const n = parseHistoryEpisode(ep);
					const atFinale = n !== null && isFinale({ status, episodes }, n);
					expect(got).toBe(atFinale && !partWay ? '7' : null);
				}
			)
		);
	});
});

describe('handoffFrom', () => {
	const none = new Set<string>();

	it("moves on to the one entry that comes next, with the series' titles", () => {
		const got = handoffFrom(payload('bleach', '43078'), '43078', 'linked', TODAY, none);
		expect(got?.finished.id).toBe('43078');
		expect(got?.next.id).toBe('46903');
		expect(got?.titles).toEqual(run('bleach').entries.map((e) => e.title));
	});

	it('keeps the ordinary card once the next entry has a history row of its own', () => {
		expect(
			handoffFrom(payload('bleach', '43078'), '43078', 'linked', TODAY, new Set(['46903']))
		).toBeNull();
	});

	it('follows the order setting: a fork as linked, one entry as released', () => {
		expect(handoffFrom(payload('fate', '7658'), '7658', 'linked', TODAY, none)).toBeNull();
		expect(handoffFrom(payload('fate', '7658'), '7658', 'released', TODAY, none)?.next.id).toBe(
			'9718'
		);
	});

	it('keeps the ordinary card before an entry not aired yet, and at the end', () => {
		expect(handoffFrom(payload('monogatari', '48434'), '48434', 'linked', TODAY, none)).toBeNull();
		expect(handoffFrom(payload('bleach', '49444'), '49444', 'linked', TODAY, none)).toBeNull();
	});

	it('waits for the series and the settings, and reads only its own series', () => {
		expect(handoffFrom(null, '43078', 'linked', TODAY, none)).toBeNull();
		expect(handoffFrom(payload('bleach', '43078'), '43078', null, TODAY, none)).toBeNull();
		expect(handoffFrom(payload('bleach', '244'), '43078', 'linked', TODAY, none)).toBeNull();
	});

	it('keeps the ordinary card for an entry outside the series it was read with', () => {
		const lone = { ...payload('bleach', '999'), entries: run('bleach').entries };
		expect(handoffFrom(lone, '999', 'linked', TODAY, none)).toBeNull();
	});

	it('names only an aired, unplayed sole next entry of its own series (property)', () => {
		fc.assert(
			fc.property(
				franchiseArb(),
				fc.constantFrom('linked', 'released') as fc.Arbitrary<'linked' | 'released'>,
				fc.array(fc.integer({ min: 0, max: 60 }).map(String), { maxLength: 4 }),
				(franchise, order, playedList) => {
					const played = new Set(playedList);
					const id = franchise.anchor;
					const got = handoffFrom(franchise, id, order, TODAY, played);
					const r = runOf(franchise);
					const up =
						r && r.entries.some((e) => e.id === id) ? upNextFor(r, id, order, TODAY) : null;
					if (up?.kind === 'one' && !played.has(up.entry.id)) {
						expect(got?.finished.id).toBe(id);
						expect(got?.next).toEqual(up.entry);
						expect(got?.titles).toEqual(r!.entries.map((e) => e.title));
					} else {
						expect(got).toBeNull();
					}
				}
			)
		);
	});
});

describe('playedIds', () => {
	it("holds each row's recorded Kitsu id, else its match's", () => {
		const got = playedIds(
			[
				{ id: 'a', kitsu_id: '1' },
				{ id: 'b' },
				{ id: 'c' },
				{ id: 'd' },
				{ id: 'e', kitsu_id: '5' }
			],
			{ a: { id: '9' }, b: { id: '2' }, c: null, e: { id: '5' } }
		);
		expect([...got!].sort()).toEqual(['1', '2', '5']);
	});
});

describe('railHandoffs', () => {
	const entry = (id: string) => ({
		id,
		title: `T${id}`,
		subtype: 'TV',
		start: '2020-01-01',
		episodes: 12,
		status: 'finished',
		poster: null,
		cover: null
	});
	const to =
		(next: string) =>
		(kitsuId: string): Handoff => ({
			finished: entry(kitsuId),
			next: entry(next),
			titles: []
		});

	it('gives each asking row its hand-off, in rail order', () => {
		const got = railHandoffs(
			[
				['r1', '10'],
				['r2', null],
				['r3', '30']
			],
			(k) => (k === '10' ? to('11')(k) : k === '30' ? to('31')(k) : null)
		);
		expect([...got.keys()]).toEqual(['r1', 'r3']);
		expect(got.get('r3')?.next.id).toBe('31');
	});

	it('moves only the first of two rows that lead to the same entry', () => {
		const got = railHandoffs(
			[
				['r1', '10'],
				['r2', '20']
			],
			to('50')
		);
		expect([...got.keys()]).toEqual(['r1']);
	});

	it('shows any entry at most once, and only for rows that asked (property)', () => {
		fc.assert(
			fc.property(
				fc.array(
					fc.tuple(fc.uuid(), fc.option(fc.integer({ min: 1, max: 9 }).map(String), { nil: null })),
					{ maxLength: 8 }
				),
				fc.func(fc.option(fc.integer({ min: 1, max: 5 }).map(String), { nil: null })),
				(asks, nextOf) => {
					const from = (k: string) => {
						const n = nextOf(k);
						return n === null ? null : to(n)(k);
					};
					const got = railHandoffs(asks, from);
					const nexts = [...got.values()].map((h) => h.next.id);
					expect(new Set(nexts).size).toBe(nexts.length);
					for (const [row, h] of got) {
						const ask = asks.find(([r]) => r === row);
						expect(ask?.[1]).toBe(h.finished.id);
					}
				}
			)
		);
	});
});
