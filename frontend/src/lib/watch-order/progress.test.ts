import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { progressFromHistory, type HistoryRow } from './progress';
import { entriesArb } from './testing/arbitraries';
import type { WatchEntry } from './types';

function entry(id: string, episodes: number | null): WatchEntry {
	return {
		id,
		title: id,
		subtype: 'TV',
		start: '2020-01-01',
		episodes,
		status: 'finished',
		poster: null,
		cover: null
	};
}

const row = (kitsu_id: string | undefined, ep_no: string): HistoryRow =>
	kitsu_id === undefined ? { ep_no } : { kitsu_id, ep_no };

describe('progressFromHistory', () => {
	const entries = [entry('7158', 26), entry('46598', 14), entry('49847', null), entry('1', 1)];

	it('reads watched at the last episode and part-way before it', () => {
		expect(
			progressFromHistory(
				[row('7158', '26'), row('46598', '13'), row('49847', '1'), row('1', '1')],
				entries
			)
		).toEqual({ '7158': 'watched', '46598': 13, '49847': 1, '1': 'watched' });
	});

	it('takes the furthest of several rows for one entry', () => {
		expect(progressFromHistory([row('46598', '3'), row('46598', '9')], entries)).toEqual({
			'46598': 9
		});
		expect(progressFromHistory([row('46598', '14'), row('46598', '2')], entries)).toEqual({
			'46598': 'watched'
		});
	});

	it('leaves out rows with no Kitsu id, no episode, or an entry outside the franchise', () => {
		expect(
			progressFromHistory(
				[row(undefined, '5'), row('46598', 'movie'), row('46598', ''), row('999', '4')],
				entries
			)
		).toEqual({});
	});

	it('reads a recap and a range the way the history does', () => {
		expect(progressFromHistory([row('46598', '7.5')], entries)).toEqual({ '46598': 7.5 });
		expect(progressFromHistory([row('46598', '1-12')], entries)).toEqual({ '46598': 1 });
	});

	it('is the furthest episode per entry, watched once it reaches a known count', () => {
		const scenario = entriesArb().chain((list) =>
			fc.record({
				entries: fc.constant(list),
				rows: fc.array(
					fc.record(
						{
							kitsu_id: fc.constantFrom(...list.map((e) => e.id), '999'),
							ep_no: fc.oneof(
								fc.integer({ min: 1, max: 40 }).map(String),
								fc.constantFrom('', 'x', '0')
							)
						},
						{ requiredKeys: ['ep_no'] }
					),
					{ maxLength: 12 }
				)
			})
		);
		fc.assert(
			fc.property(scenario, ({ entries, rows }) => {
				const got = progressFromHistory(rows, entries);
				for (const e of entries) {
					const eps = rows
						.filter((r) => r.kitsu_id === e.id)
						.map((r) => Number(r.ep_no))
						.filter((n) => n > 0);
					if (eps.length === 0) {
						expect(got[e.id]).toBeUndefined();
						continue;
					}
					const furthest = Math.max(...eps);
					const done = e.episodes !== null && furthest >= e.episodes;
					expect(got[e.id]).toBe(done ? 'watched' : furthest);
				}
				expect(Object.keys(got).every((id) => entries.some((e) => e.id === id))).toBe(true);
			})
		);
	});

	it('does not depend on the order of the rows', () => {
		const list = [entry('1', 12), entry('2', null), entry('3', 3)];
		const rows = fc.array(
			fc.record({
				kitsu_id: fc.constantFrom('1', '2', '3'),
				ep_no: fc.integer({ min: 1, max: 15 }).map(String)
			}),
			{ maxLength: 10 }
		);
		fc.assert(
			fc.property(
				rows.chain((r) =>
					fc.tuple(
						fc.constant(r),
						fc.shuffledSubarray(r, { minLength: r.length, maxLength: r.length })
					)
				),
				([a, b]) => {
					expect(progressFromHistory(b, list)).toEqual(progressFromHistory(a, list));
				}
			)
		);
	});
});
