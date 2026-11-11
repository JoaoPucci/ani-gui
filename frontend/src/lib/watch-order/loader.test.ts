import { describe, expect, it, vi } from 'vitest';
import fc from 'fast-check';
import { loadWatchOrder, type WatchOrderSources } from './loader';
import type { HistoryRow } from './progress';
import type { FranchiseData, WatchEntry } from './types';

const entry = (id: string): WatchEntry => ({
	id,
	title: `Entry ${id}`,
	subtype: 'TV',
	start: '2020-01-01',
	episodes: 12,
	status: 'finished',
	poster: null,
	cover: null
});

const FRANCHISE: FranchiseData = {
	anchor: '7',
	entries: [entry('7'), entry('8')],
	links: [['7', '8']],
	versions: [entry('20')],
	films: [entry('30')],
	sides: [entry('40')],
	recaps: [entry('50')],
	parents: {},
	complete: true
};
const NAMED: Record<string, HistoryRow | null> = { '7': { kitsu_id: '7', ep_no: '3' }, '8': null };
const ROWS: HistoryRow[] = [{ kitsu_id: '7', ep_no: '3' }];

const sources = (franchise: boolean, history: boolean): WatchOrderSources => ({
	franchise: () =>
		franchise ? Promise.resolve(FRANCHISE) : Promise.reject(new Error('kitsu unreachable')),
	named: () => (history ? Promise.resolve(NAMED) : Promise.reject(new Error('no backend')))
});

describe('loadWatchOrder', () => {
	it('hands on the franchise and the rows its entries resume from', async () => {
		expect(await loadWatchOrder('7', sources(true, true))).toEqual({
			franchise: FRANCHISE,
			rows: ROWS
		});
	});

	it('asks for the franchise of the entry the page shows', async () => {
		const franchise = vi.fn((id: string) => {
			void id;
			return Promise.resolve(FRANCHISE);
		});
		await loadWatchOrder('42', { franchise, named: () => Promise.resolve({}) });
		expect(franchise).toHaveBeenCalledTimes(1);
		expect(franchise.mock.calls[0][0]).toBe('42');
	});

	it('asks for the row of every entry the franchise names, once each', async () => {
		const named = vi.fn((ids: readonly string[]) => {
			void ids;
			return Promise.resolve({});
		});
		await loadWatchOrder('7', { franchise: () => Promise.resolve(FRANCHISE), named });
		expect(named).toHaveBeenCalledTimes(1);
		expect([...named.mock.calls[0][0]].sort()).toEqual(['20', '30', '40', '50', '7', '8']);
	});

	it('counts a row toward the entry that named it, whatever id the row recorded', async () => {
		// The row recorded entry 4, which Kitsu has since deleted; the
		// backend names it for 8, its replacement (history_claim::names).
		const got = await loadWatchOrder('7', {
			franchise: () => Promise.resolve(FRANCHISE),
			named: () => Promise.resolve({ '8': { kitsu_id: '4', ep_no: '5' } })
		});
		expect(got.rows).toEqual([{ kitsu_id: '8', ep_no: '5' }]);
	});

	it('reads a franchise that cannot be read as none, and asks for no rows', async () => {
		const named = vi.fn(() => Promise.resolve(NAMED));
		expect(
			await loadWatchOrder('7', {
				franchise: () => Promise.reject(new Error('kitsu unreachable')),
				named
			})
		).toEqual({ franchise: null, rows: [] });
		expect(named).not.toHaveBeenCalled();
	});

	it('reads a history that cannot be read as empty: a strip without progress', async () => {
		expect(await loadWatchOrder('7', sources(true, false))).toEqual({
			franchise: FRANCHISE,
			rows: []
		});
	});

	it('never rejects, whichever read fails', async () => {
		await fc.assert(
			fc.asyncProperty(fc.boolean(), fc.boolean(), async (f, h) => {
				const got = await loadWatchOrder('7', sources(f, h));
				expect(got.franchise).toBe(f ? FRANCHISE : null);
				expect(got.rows).toEqual(f && h ? ROWS : []);
			})
		);
	});

	it("hands the visit's signal to both reads", async () => {
		const visit = new AbortController();
		const franchise = vi.fn((id: string, signal?: AbortSignal) => {
			void id;
			void signal;
			return Promise.resolve(FRANCHISE);
		});
		const named = vi.fn((ids: readonly string[], signal?: AbortSignal) => {
			void ids;
			void signal;
			return Promise.resolve({});
		});
		await loadWatchOrder('7', { franchise, named }, visit.signal);
		expect(franchise.mock.calls[0][1]).toBe(visit.signal);
		expect(named.mock.calls[0][1]).toBe(visit.signal);
	});

	it('asks for no rows once the visit has ended, and answers nothing', async () => {
		// The franchise answered just as the viewer left: the rows would
		// only take a connection for a page that is gone.
		const visit = new AbortController();
		const named = vi.fn(() => Promise.resolve(NAMED));
		const got = await loadWatchOrder(
			'7',
			{
				franchise: () => {
					visit.abort();
					return Promise.resolve(FRANCHISE);
				},
				named
			},
			visit.signal
		);
		expect(named).not.toHaveBeenCalled();
		expect(got).toEqual({ franchise: null, rows: [] });
	});

	it('answers nothing for a visit that ended while the rows were out', async () => {
		// Not a strip without progress: the rows were cut off, not
		// unreadable, and nothing may keep this answer as the show's.
		const visit = new AbortController();
		const got = await loadWatchOrder(
			'7',
			{
				franchise: () => Promise.resolve(FRANCHISE),
				named: () => {
					visit.abort();
					return Promise.reject(new DOMException('aborted', 'AbortError'));
				}
			},
			visit.signal
		);
		expect(got).toEqual({ franchise: null, rows: [] });
	});

	it('starts no read for a visit that has already ended', async () => {
		const visit = new AbortController();
		visit.abort();
		const franchise = vi.fn(() => Promise.resolve(FRANCHISE));
		const named = vi.fn(() => Promise.resolve(NAMED));
		const got = await loadWatchOrder('7', { franchise, named }, visit.signal);
		expect(franchise).not.toHaveBeenCalled();
		expect(named).not.toHaveBeenCalled();
		expect(got).toEqual({ franchise: null, rows: [] });
	});

	it('answers nothing whenever the visit ends, and never rejects', async () => {
		// The visit ends before the franchise read, during it, between
		// the reads, during the rows read, or not at all.
		await fc.assert(
			fc.asyncProperty(
				fc.integer({ min: 0, max: 4 }),
				fc.boolean(),
				fc.boolean(),
				async (at, f, h) => {
					const visit = new AbortController();
					const end = (step: number) => {
						if (at === step) visit.abort();
					};
					end(0);
					const got = await loadWatchOrder(
						'7',
						{
							franchise: async () => {
								end(1);
								if (!f) throw new Error('kitsu unreachable');
								end(2);
								return FRANCHISE;
							},
							named: async () => {
								end(3);
								if (!h) throw new Error('no backend');
								return NAMED;
							}
						},
						visit.signal
					);
					if (visit.signal.aborted) expect(got).toEqual({ franchise: null, rows: [] });
					else expect(got).toEqual({ franchise: f ? FRANCHISE : null, rows: f && h ? ROWS : [] });
				}
			)
		);
	});

	it('gives each named entry its row and nothing else', async () => {
		const ids = ['7', '8', '20', '30', '40', '50'];
		const answer = fc.dictionary(
			fc.constantFrom(...ids),
			fc.option(
				fc.record(
					{
						kitsu_id: fc.constantFrom('4', '7', '99'),
						ep_no: fc.integer({ min: 1, max: 30 }).map(String)
					},
					{ requiredKeys: ['ep_no'] }
				),
				{ nil: null }
			)
		);
		await fc.assert(
			fc.asyncProperty(answer, async (named) => {
				const got = await loadWatchOrder('7', {
					franchise: () => Promise.resolve(FRANCHISE),
					named: () => Promise.resolve(named)
				});
				const want = Object.entries(named).flatMap(([id, r]) =>
					r ? [{ kitsu_id: id, ep_no: r.ep_no }] : []
				);
				expect(got.rows).toEqual(want);
			})
		);
	});
});
